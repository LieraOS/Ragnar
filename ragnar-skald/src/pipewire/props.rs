//! SPA `Props` parameter pod construction and parsing.
//!
//! PipeWire exposes volume/mute on nodes as `ParamType::Props` objects.
//! This module is the only place that knows how those pods look; both
//! directions (serialize for `set_param`, deserialize for param events)
//! are covered by unit tests so the wire format is verified locally
//! without a running daemon.

use std::io::Cursor;

use libspa::param::ParamType;
use libspa::pod::deserialize::PodDeserializer;
use libspa::pod::serialize::PodSerializer;
use libspa::pod::{Object, Pod, Property, Value, ValueArray};
use libspa::sys;
use libspa::utils::SpaTypes;

/// Parsed subset of a `Props` param relevant for volume control.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PropsState {
    /// Master volume (`volume` prop) — linear gain, 1.0 = 100 %.
    pub volume: Option<f32>,
    /// Per-channel volumes (`channelVolumes` prop).
    pub channel_volumes: Option<Vec<f32>>,
    /// Mute flag (`mute` prop).
    pub mute: Option<bool>,
}

/// Serialize a `Props` pod that sets volume and mute on a node.
///
/// `channel_volumes` must contain one gain per channel — nodes ignore a
/// master `volume` alone for multi-channel devices, while simple
/// streams ignore `channelVolumes`. Sending both covers all node types.
pub fn build_volume_pod(
    channel_volumes: &[f32],
    mute: Option<bool>,
) -> Result<Vec<u8>, libspa::pod::serialize::GenError> {
    let mut properties = vec![Property::new(
        sys::SPA_PROP_channelVolumes,
        Value::ValueArray(ValueArray::Float(channel_volumes.to_vec())),
    )];

    // The scalar `volume` mirrors the channel average for tools that
    // display it and for single-volume nodes.
    let master = channel_volumes.iter().sum::<f32>() / channel_volumes.len().max(1) as f32;
    properties.push(Property::new(sys::SPA_PROP_volume, Value::Float(master)));

    if let Some(mute) = mute {
        properties.push(Property::new(sys::SPA_PROP_mute, Value::Bool(mute)));
    }

    serialize_props(properties)
}

/// Serialize a `Props` pod that only toggles mute without touching volume.
pub fn build_mute_pod(mute: bool) -> Result<Vec<u8>, libspa::pod::serialize::GenError> {
    serialize_props(vec![Property::new(sys::SPA_PROP_mute, Value::Bool(mute))])
}

fn serialize_props(properties: Vec<Property>) -> Result<Vec<u8>, libspa::pod::serialize::GenError> {
    let (cursor, _) = PodSerializer::serialize(
        Cursor::new(Vec::new()),
        &Value::Object(Object {
            type_: SpaTypes::ObjectParamProps.as_raw(),
            id: ParamType::Props.as_raw(),
            properties,
        }),
    )?;
    Ok(cursor.into_inner())
}

/// Serialize a `Props` pod carrying a `params` struct — the mechanism
/// filter-chain graphs use for runtime control changes: pairs of
/// control name → numeric value routed to the graph's first node.
pub fn build_control_pod(
    controls: &[(&str, f32)],
) -> Result<Vec<u8>, libspa::pod::serialize::GenError> {
    let mut fields = Vec::with_capacity(controls.len() * 2);
    for (name, value) in controls {
        fields.push(Value::String((*name).to_string()));
        fields.push(Value::Float(*value));
    }
    serialize_props(vec![Property::new(
        sys::SPA_PROP_params,
        Value::Struct(fields),
    )])
}

/// Parse a `Props` param pod received in a node's `param` event.
///
/// Returns `Ok(None)` when the pod is not a `Props` object (e.g. a
/// different param id sneaks through) so callers can ignore it.
pub fn parse_props_pod(pod: &Pod) -> Option<PropsState> {
    let (_, value) = PodDeserializer::deserialize_any_from(pod.as_bytes()).ok()?;
    let object = match value {
        Value::Object(object) if object.type_ == SpaTypes::ObjectParamProps.as_raw() => object,
        _ => return None,
    };

    let mut state = PropsState::default();
    for prop in object.properties {
        match (prop.key, prop.value) {
            (sys::SPA_PROP_volume, Value::Float(v)) => state.volume = Some(v),
            (sys::SPA_PROP_channelVolumes, Value::ValueArray(ValueArray::Float(v))) => {
                state.channel_volumes = Some(v);
            }
            (sys::SPA_PROP_mute, Value::Bool(m)) => state.mute = Some(m),
            _ => {}
        }
    }
    Some(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use libspa::pod::Pod;

    fn roundtrip(channel_volumes: &[f32], mute: Option<bool>) -> PropsState {
        let bytes = build_volume_pod(channel_volumes, mute).expect("serialize failed");
        let pod = Pod::from_bytes(&bytes).expect("invalid pod bytes");
        parse_props_pod(pod).expect("not a props pod")
    }

    #[test]
    fn volume_pod_roundtrips_stereo() {
        let state = roundtrip(&[0.5, 0.75], None);
        assert_eq!(state.channel_volumes.as_deref(), Some(&[0.5f32, 0.75][..]));
        assert_eq!(state.volume, Some(0.625));
        assert_eq!(state.mute, None);
    }

    #[test]
    fn volume_pod_roundtrips_mono() {
        let state = roundtrip(&[1.5], None);
        assert_eq!(state.channel_volumes.as_deref(), Some(&[1.5f32][..]));
        assert_eq!(state.volume, Some(1.5));
    }

    #[test]
    fn volume_pod_carries_mute() {
        let state = roundtrip(&[1.0, 1.0], Some(true));
        assert_eq!(state.mute, Some(true));
    }

    #[test]
    fn mute_pod_has_no_volume() {
        let bytes = build_mute_pod(false).expect("serialize failed");
        let pod = Pod::from_bytes(&bytes).expect("invalid pod bytes");
        let state = parse_props_pod(pod).expect("not a props pod");
        assert_eq!(state.mute, Some(false));
        assert_eq!(state.channel_volumes, None);
        assert_eq!(state.volume, None);
    }

    #[test]
    fn parse_rejects_non_props_pod() {
        // A plain string pod must not be mistaken for a Props object.
        let (cursor, _) =
            PodSerializer::serialize(Cursor::new(Vec::new()), &Value::String("x".into()))
                .expect("serialize failed");
        let bytes = cursor.into_inner();
        let pod = Pod::from_bytes(&bytes).expect("invalid pod bytes");
        assert!(parse_props_pod(pod).is_none());
    }
}
