// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-shared — D-Bus interface definition for the display service
// Copyright (C) 2026 LieraOS Project

use serde::{Deserialize, Serialize};
use zbus::zvariant::Type;

use crate::display::Head;

/// Return value for GetOutputs: list of all currently connected heads.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetOutputsReply {
    pub heads: Vec<Head>,
}

/// Argument for SetPosition: move an output to a new position.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SetPositionArgs {
    pub output: String,
    pub x: i32,
    pub y: i32,
}

/// Argument for SetPrimary: designate an output as primary.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SetPrimaryArgs {
    pub output: String,
}

/// Argument for SetMode: change resolution/refresh of an output.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SetModeArgs {
    pub output: String,
    pub width: i32,
    pub height: i32,
    pub refresh_rate: i32,
}

/// Argument for EnableOutput: turn an output on or off.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EnableOutputArgs {
    pub output: String,
    pub enabled: bool,
}

/// Generic success/error reply for write operations.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ResultReply {
    pub success: bool,
    pub message: String,
}
