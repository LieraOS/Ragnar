// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — WorkflowAutomation,.
//
// CRUD + templates match the ; the step executor is REAL —
// stubbed every action with `return true`. Steps dispatch
// through a handler map so the UI/workflow layer supplies actual
// implementations; SendEmail calls the lettre-based mail module.

use crate::mail::send::{self, EmailConfig};
use std::collections::HashMap;

/// Step kinds — WorkflowAction enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkflowAction {
 /// Acquire a scan.
 Scan,
 /// Convert the payload to PDF.
 ConvertToPdf,
 /// Mail the payload.
 SendEmail,
 /// Write the payload to disk.
 SaveLocally,
 /// Print the payload.
 PrintDocument,
}

/// One enabled/disabled step — WorkflowStep.
#[derive(Debug, Clone)]
pub struct WorkflowStep {
 /// Action kind.
 pub action: WorkflowAction,
 /// Action parameter (path, recipient,.).
 pub value: String,
 /// Step enabled flag.
 pub enabled: bool,
}

/// A named automation — AutomationWorkflow.
#[derive(Debug, Clone, Default)]
pub struct AutomationWorkflow {
 /// Stable id ("scan_to_pdf").
 pub workflow_id: String,
 /// Display name.
 pub workflow_name: String,
 /// Ordered steps.
 pub steps: Vec<WorkflowStep>,
 /// Trigger flags (manual/scheduled/hotkey) — carried like.
 pub enabled: bool,
 /// "manual" | "scheduled" | "hotkey".
 pub trigger_type: String,
 /// Trigger parameter.
 pub trigger_value: String,
}

/// Step executor — callers register real handlers per action;
/// returns false when a step fails, exactly the design.
type StepHandler = Box<dyn Fn(&WorkflowStep, &str) -> bool + Send>;

/// Automation engine.
#[derive(Default)]
pub struct WorkflowAutomation {
 workflows: Vec<AutomationWorkflow>,
 email_config: EmailConfig,
 handlers: HashMap<WorkflowAction, StepHandler>,
}

impl WorkflowAutomation {
 /// New empty engine.
 pub fn new() -> Self {
 Self::default()
 }

 /// Register a real handler for an action — without a handler a
 /// step reports failure instead of pretending success (the 
 /// returned true unconditionally; that was a stub).
 pub fn register_handler(&mut self, action: WorkflowAction, handler: StepHandler) {
 self.handlers.insert(action, handler);
 }

 /// Append a workflow — createWorkflow.
 pub fn create_workflow(&mut self, wf: AutomationWorkflow) -> bool {
 self.workflows.push(wf);
 true
 }

 /// Replace by id — updateWorkflow.
 pub fn update_workflow(&mut self, wf: AutomationWorkflow) -> bool {
 for w in self.workflows.iter_mut() {
 if w.workflow_id == wf.workflow_id {
 *w = wf;
 return true;
 }
 }
 false
 }

 /// Delete by id — deleteWorkflow.
 pub fn delete_workflow(&mut self, id: &str) -> bool {
 let before = self.workflows.len();
 self.workflows.retain(|w| w.workflow_id != id);
 self.workflows.len() != before
 }

 /// Lookup by id — getWorkflow.
 pub fn get_workflow(&self, id: &str) -> Option<&AutomationWorkflow> {
 self.workflows.iter().find(|w| w.workflow_id == id)
 }

 /// All workflows.
 pub fn all(&self) -> &[AutomationWorkflow] {
 &self.workflows
 }

 /// Store EmailConfig — configureEmail.
 pub fn configure_email(&mut self, config: EmailConfig) -> bool {
 self.email_config = config;
 true
 }

 /// Current email config.
 pub fn email_config(&self) -> &EmailConfig {
 &self.email_config
 }

 /// Run a workflow over an input file — executeWorkflow.
 /// Each enabled step dispatches through its handler (or the
 /// real mail path for SendEmail); first failure stops the run.
 pub fn execute_workflow(&self, id: &str, input_file: &str) -> bool {
 let wf = match self.get_workflow(id) {
 Some(w) => w,
 None => return false,
 };
 for step in &wf.steps {
 if !step.enabled {
 continue;
 }
 if !self.execute_step(step, input_file) {
 return false;
 }
 }
 true
 }

 /// Dispatch one step — registered handler wins; SendEmail has a
 /// built-in real implementation via the mail module.
 fn execute_step(&self, step: &WorkflowStep, input_file: &str) -> bool {
 if let Some(h) = self.handlers.get(&step.action) {
 return h(step, input_file);
 }
 match step.action {
 WorkflowAction::SendEmail => {
 send::send_with_attachment(input_file, &self.email_config).is_ok()
 }
 // Scan/ConvertToPDF/SaveLocally/PrintDocument require a
 // registered handler — without one the step honestly
 // fails instead of faking success like the.
 _ => false,
 }
 }

 /// The two built-in templates — getScanToPDFTemplate and
 /// getScanToPDFEmailTemplate from the.
 pub fn templates() -> Vec<AutomationWorkflow> {
 vec![
 AutomationWorkflow {
 workflow_id: "scan_to_pdf".into(),
 workflow_name: "Scan to PDF".into(),
 steps: vec![
 WorkflowStep { action: WorkflowAction::Scan, value: String::new(), enabled: true },
 WorkflowStep { action: WorkflowAction::ConvertToPdf, value: String::new(), enabled: true },
 WorkflowStep { action: WorkflowAction::SaveLocally, value: String::new(), enabled: true },
 ],
 enabled: true,
..Default::default()
 },
 AutomationWorkflow {
 workflow_id: "scan_to_pdf_email".into(),
 workflow_name: "Scan to PDF & Email".into(),
 steps: vec![
 WorkflowStep { action: WorkflowAction::Scan, value: String::new(), enabled: true },
 WorkflowStep { action: WorkflowAction::ConvertToPdf, value: String::new(), enabled: true },
 WorkflowStep { action: WorkflowAction::SendEmail, value: String::new(), enabled: true },
 ],
 enabled: true,
..Default::default()
 },
 ]
 }
}
