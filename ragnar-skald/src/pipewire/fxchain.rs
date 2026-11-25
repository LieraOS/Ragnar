// Copyright (C) Jörg Schröder
//! Filter-chain effects loaded as PipeWire modules.
//!
//! Skald ships a small set of built-in effects so users do not need
//! EasyEffects for everyday use. Each effect is a separate
//! `libpipewire-module-filter-chain` instance loaded into our own
//! PipeWire context (same mechanism EasyEffects uses). The modules are
//! chained via `target.object`: applications feed the first module's
//! capture sink, the last module's playback goes to whatever the session
//! default sink is — the chain follows output-device changes.
//!
//! One module per effect is required because PipeWire routes `params`
//! control updates to the first node of a graph only; with one effect
//! per module every slider is fully controllable at runtime.
//!
//! Module layout per effect (`capture` presents an `Audio/Sink`, so it
//! shows up as a normal output device and even keeps volume/mute
//! properties):
//!
//! ```text
//! apps → bass.in ─→ bass.out ─→ treble.in → treble.out
//!      → echo.in  ─→ echo.out ─→ reverb.in → reverb.out
//!      → gain.in  ─→ gain.out ─→ <session default sink>
//! ```

use std::ffi::CString;

use pipewire::context::Context;
use pipewire::sys as pw_sys;
use tracing::{debug, warn};

use super::types::FxEffect;

/// Node name prefix used to recognize Skald-owned fx nodes in the
/// registry (and to keep them out of the user-facing device list).
pub const FX_NODE_PREFIX: &str = "skald.fx.";

/// `true` when a node name belongs to our filter-chain modules.
pub fn is_fx_node(name: &str) -> bool {
    name.starts_with(FX_NODE_PREFIX)
}

/// Map a capture node name (`skald.fx.<key>.in`) back to its
/// effect — returns `None` for foreign or playback (`*.out`) nodes.
pub fn effect_for_capture(name: &str) -> Option<FxEffect> {
    let key = name.strip_prefix(FX_NODE_PREFIX)?.strip_suffix(".in")?;
    FxEffect::from_key(key)
}

/// Control updates are routed to the *capture* node of the module.
pub fn capture_node_name(effect: FxEffect) -> String {
    format!("{FX_NODE_PREFIX}{}.in", effect.key())
}

/// Stable chain order: bass → treble → echo → reverb.
/// Reverb is last so its playback (no target) lands on the session
/// default sink and follows output-device changes.
const CHAIN_ORDER: &[FxEffect] = &[
    FxEffect::Bass,
    FxEffect::Treble,
    FxEffect::Echo,
    FxEffect::Reverb,
];

/// Build the module `args` string (SPA-JSON) for one effect.
///
/// `next` is the capture node name of the following module in the
/// chain; `None` means "link to session default" (handled by the
/// session manager automatically — no `target.object` needed).
pub fn module_args(effect: FxEffect, next: Option<&str>, ir_path: &str) -> String {
    let next = next.unwrap_or("");
    let target = if next.is_empty() {
        String::new()
    } else {
        format!("target.object = \"{next}\"")
    };
    let key = effect.key();
    let graph = graph_config(effect, ir_path);

    format!(
        "{{ node.description = \"Skald: {label}\" \
            filter.graph = {graph} \
            audio.channels = 2 \
            audio.position = [ FL FR ] \
            capture.props = {{ \
                node.name = \"{prefix}{key}.in\" \
                node.description = \"Skald Effekte\" \
                media.class = Audio/Sink \
            }} \
            playback.props = {{ \
                node.name = \"{prefix}{key}.out\" \
                node.passive = true \
                {target} \
            }} \
        }}",
        label = effect.label(),
        graph = graph,
        prefix = FX_NODE_PREFIX,
        key = key,
    )
}

/// `filter.graph` node/link config per effect.
///
/// Controls that must be adjustable at runtime live on the **first**
/// node of the graph — PipeWire routes `params` to `node_list` head.
fn graph_config(effect: FxEffect, ir_path: &str) -> String {
    match effect {
        FxEffect::Bass => single_node("bq_lowshelf", r#""Freq" = 100.0 "Q" = 1.0 "Gain" = 0.0"#),
        FxEffect::Treble => {
            single_node("bq_highshelf", r#""Freq" = 3000.0 "Q" = 1.0 "Gain" = 0.0"#)
        }
        // Echo, wet/dry per channel. The graph engine duplicates the
        // whole node set per channel when the graph exposes a single
        // input/output (`n_hndl` fan-out), so the graph below is mono:
        // a `copy` tee feeds the dry mixer input and the delay; the
        // delay output is the wet path.
        //
        // PipeWire ≤ 1.4 compatibility notes:
        // - `delay` requires a `config` object with `max-delay` —
        //   instantiation fails with EINVAL otherwise.
        // - Its only control port is `Delay (s)` — `Feedback`/
        //   `Feedforward` exist only in newer versions.
        // So `fx_mix` is the first node instead: runtime `params`
        // controls are routed to the first node only, and mixer
        // `Gain 2` (the wet level) is our echo intensity.
        FxEffect::Echo => echo_graph(),
        // Reverb, same topology: `fx_mix` is first so its wet gain
        // ("Gain 2") is the runtime-controllable param; the convolver
        // itself has no runtime controls — it only needs the IR file.
        FxEffect::Reverb => format!(
            "{{ nodes = [ \
                {{ type = builtin name = fx_mix label = mixer \
                   control = {{ \"Gain 1\" = 1.0 \"Gain 2\" = 0.0 }} }} \
                {{ type = builtin name = fx_tee label = copy }} \
                {{ type = builtin name = fx_verb label = convolver \
                   config = {{ filename = \"{ir_path}\" }} }} \
            ] \
            inputs  = [ \"fx_tee:In\" ] \
            outputs = [ \"fx_mix:Out\" ] \
            links = [ \
                {{ output = \"fx_tee:Out\"  input = \"fx_mix:In 1\" }} \
                {{ output = \"fx_tee:Out\"  input = \"fx_verb:In\" }} \
                {{ output = \"fx_verb:Out\" input = \"fx_mix:In 2\" }} \
            ] }}"
        ),
    }
}

fn single_node(label: &str, control: &str) -> String {
    format!(
        "{{ nodes = [ {{ type = builtin name = fx label = {label} \
          control = {{ {control} }} }} ] }}"
    )
}

/// Shared mono wet/dry echo graph — used by both the output chain
/// (`FxEffect::Echo`) and the microphone chain (`MicEffect::Echo`).
/// `fx_mix` must stay the first node: PipeWire routes runtime `params`
/// to the graph head only, and mixer `Gain 2` is the intensity.
///
/// PipeWire ≤ 1.4 notes: `delay` requires `config = { max-delay }` and
/// has `Delay (s)` as its only control port (no Feedback/Feedforward).
pub(crate) fn echo_graph() -> String {
    String::from(
        "{ nodes = [ \
            { type = builtin name = fx_mix label = mixer \
              control = { \"Gain 1\" = 1.0 \"Gain 2\" = 0.0 } } \
            { type = builtin name = fx_tee label = copy } \
            { type = builtin name = fx_delay label = delay \
              config = { \"max-delay\" = 1.0 } \
              control = { \"Delay (s)\" = 0.22 } } \
        ] \
        inputs  = [ \"fx_tee:In\" ] \
        outputs = [ \"fx_mix:Out\" ] \
        links = [ \
            { output = \"fx_tee:Out\"   input = \"fx_delay:In\" } \
            { output = \"fx_tee:Out\"   input = \"fx_mix:In 1\" } \
            { output = \"fx_delay:Out\" input = \"fx_mix:In 2\" } \
        ] }",
    )
}

/// Load one filter-chain module into our context (FFI — the `pipewire`
/// crate does not wrap `pw_context_load_module`).
///
/// Returns the raw module pointer used for later `destroy`, or `None`
/// when the daemon rejected the module.
pub fn load_module(
    context: &Context,
    effect: FxEffect,
    next_capture: Option<&str>,
    ir_path: &str,
) -> Option<*mut pw_sys::pw_impl_module> {
    let args = module_args(effect, next_capture, ir_path);
    load_module_raw(context, "libpipewire-module-filter-chain", &args)
}

/// Load an arbitrary PipeWire module into our client context.
///
/// Shared low-level helper used for `filter-chain`, `loopback` and the
/// microphone chain. `args` is an SPA-JSON dict as accepted by
/// `pw_context_load_module`.
pub fn load_module_raw(
    context: &Context,
    name: &str,
    args: &str,
) -> Option<*mut pw_sys::pw_impl_module> {
    let name = match CString::new(name) {
        Ok(n) => n,
        Err(e) => {
            warn!("Invalid module name: {e}");
            return None;
        }
    };
    let args = match CString::new(args) {
        Ok(a) => a,
        Err(e) => {
            warn!("Invalid module args: {e}");
            return None;
        }
    };

    // SAFETY: `context` is a valid live context on the worker thread,
    // name/args are NUL-terminated CStrings that outlive the call, and
    // properties is allowed to be NULL.
    let ptr = unsafe {
        pw_sys::pw_context_load_module(
            context.as_raw_ptr(),
            name.as_ptr(),
            args.as_ptr(),
            std::ptr::null_mut(),
        )
    };
    if ptr.is_null() {
        warn!("Failed to load PipeWire module {name:?}");
        return None;
    }
    debug!("Loaded PipeWire module {name:?}");
    Some(ptr)
}

/// Destroy a previously loaded module (removes its nodes/streams).
///
/// # Safety
///
/// `ptr` must be a pointer returned by `load_module` on the same
/// context and not yet destroyed.
pub unsafe fn destroy_module(ptr: *mut pw_sys::pw_impl_module) {
    pw_sys::pw_impl_module_destroy(ptr);
}

/// The ordered effect chain — used to wire `target.object` links.
pub fn chain_order() -> &'static [FxEffect] {
    CHAIN_ORDER
}
