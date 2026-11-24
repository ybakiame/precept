//! Model-owned rule-check state and stale-completion fencing. This module does
//! no transport or filesystem work; the native UI starts I/O only on a press.
use super::{LintReport, report::INVALID};
use crate::{
    Model,
    connection::{Connection, Target},
    projects,
};
use serde_json::Value;

/// The identity of one operator `rules.lint` request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleLintRequest {
    pub epoch: u64,
    pub project: String,
    pub serial: u64,
    pub connection: Connection,
}

/// Retained diagnostics and the copy-only template dialog selection.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuleLintState {
    pub busy: bool,
    pub serial: u64,
    pub project: Option<String>,
    pub report: Option<LintReport>,
    /// A fixed UI reason only; daemon and transport error text is never kept.
    pub error: Option<&'static str>,
    pub selected_template: Option<crate::rule_templates::RuleTemplate>,
}

impl RuleLintState {
    /// Forgets the current scope's result and rejects every older completion.
    pub fn invalidate(&mut self) {
        self.serial = self.serial.wrapping_add(1);
        self.busy = false;
        self.project = None;
        self.report = None;
        self.error = None;
        self.selected_template = None;
    }

    /// Opens the template dialog on Rust, or keeps the operator's last choice.
    pub fn open_templates(&mut self) {
        if self.selected_template.is_none() {
            self.selected_template = Some(crate::rule_templates::RuleTemplate::Rust);
        }
    }

    /// Closes the template dialog without writing anything.
    pub fn close_templates(&mut self) {
        self.selected_template = None;
    }

    fn begin(&mut self, epoch: u64, project: String, connection: Connection) -> RuleLintRequest {
        self.serial = self.serial.wrapping_add(1);
        self.busy = true;
        self.project = Some(project.clone());
        self.report = None;
        self.error = None;
        RuleLintRequest {
            epoch,
            project,
            serial: self.serial,
            connection,
        }
    }

    fn current(&self, request: &RuleLintRequest) -> bool {
        self.busy
            && self.serial == request.serial
            && self.project.as_deref() == Some(request.project.as_str())
    }

    fn finish(result: Result<Value, &'static str>) -> Result<LintReport, &'static str> {
        let value = result.map_err(|_| "Unavailable")?;
        LintReport::parse(value).map_err(|_| INVALID)
    }
}

impl Model {
    /// Whether the selected scope can issue an operator-only lint read now.
    pub fn rule_lint_ready(&self) -> bool {
        self.live
            && self.phase == crate::Phase::Ready
            && matches!(self.connection.target, Target::Local | Target::Wsl)
            && self.connection.validate()
            && self
                .selected_project
                .as_deref()
                .is_some_and(projects::valid_name)
            && self.project().is_some_and(|project| {
                Some(project.name.as_str()) == self.selected_project.as_deref()
            })
    }

    /// Starts one lint read and returns the exact identity its completion must
    /// carry. A newer request supersedes an older one even if it is still busy.
    pub fn request_rule_lint(&mut self) -> Result<RuleLintRequest, &'static str> {
        if !self.rule_lint_ready() {
            return Err(if self.live {
                "InvalidRequest"
            } else {
                "Unavailable"
            });
        }
        let project = self.selected_project.clone().ok_or("InvalidRequest")?;
        Ok(self
            .rule_lint
            .begin(self.epoch, project, self.connection.clone()))
    }

    /// Settles only the newest request for the same read epoch, applied
    /// connection and exact project. All failures become fixed UI reasons.
    pub fn complete_rule_lint(
        &mut self,
        request: &RuleLintRequest,
        result: Result<Value, &'static str>,
    ) -> bool {
        if request.epoch != self.epoch
            || request.connection != self.connection
            || self.selected_project.as_deref() != Some(request.project.as_str())
            || !self.rule_lint_ready()
            || !self.rule_lint.current(request)
        {
            return false;
        }
        let parsed = RuleLintState::finish(result);
        self.rule_lint.busy = false;
        match parsed {
            Ok(report) => {
                self.rule_lint.report = Some(report);
                self.rule_lint.error = None;
            }
            Err(error) => {
                self.rule_lint.report = None;
                self.rule_lint.error = Some(error);
            }
        }
        true
    }
}
