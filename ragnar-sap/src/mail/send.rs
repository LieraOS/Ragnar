// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — SMTP mail with attachment, real implementation of
// sendEmailWithAttachment (a placeholder).
//
// lettre handles the wire protocol; the config maps 1:1 to the
// email config fields (server, 587, sender, password,
// recipient, subject, body, useTLS). Password never gets logged.

use lettre::message::{header::ContentType, Attachment, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

/// Email configuration — same fields as EmailConfig.
#[derive(Debug, Clone, Default)]
pub struct EmailConfig {
 /// SMTP relay host.
 pub smtp_server: String,
 /// — 587 STARTTLS default.
 pub smtp_port: u16,
 /// From address / login user.
 pub sender_email: String,
 /// SMTP auth password (never logged).
 pub sender_password: String,
 /// To address.
 pub recipient_email: String,
 /// Subject line.
 pub subject: String,
 /// Plain-text body.
 pub body: String,
 /// STARTTLS when true; plaintext when false.
 pub use_tls: bool,
}

/// Typed error for send failures.
#[derive(Debug)]
pub struct MailError(pub String);

impl std::fmt::Display for MailError {
 fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
 write!(f, "{}", self.0)
 }
}

impl std::error::Error for MailError {}

/// Send the file at `file_path` as an attachment.
/// TLS selection follows useTLS flag: lettre's relay
/// = STARTTLS, builder_dangerous = plaintext.
pub fn send_with_attachment(file_path: &str, config: &EmailConfig) -> Result<(), MailError> {
 if config.smtp_server.is_empty() || config.sender_email.is_empty() || config.recipient_email.is_empty() {
 return Err(MailError("incomplete email config".into()));
 }
 let data = std::fs::read(file_path).map_err(|e| MailError(e.to_string()))?;
 let filename = std::path::Path::new(file_path)
.file_name()
.map(|n| n.to_string_lossy().into_owned())
.unwrap_or_else(|| "attachment.bin".into());
 let mime: ContentType = "application/pdf"
.parse()
.unwrap_or_else(|_| "application/octet-stream".parse().unwrap());
 let email = Message::builder()
.from(config.sender_email.parse().map_err(|e: lettre::address::AddressError| MailError(e.to_string()))?)
.to(config.recipient_email.parse().map_err(|e: lettre::address::AddressError| MailError(e.to_string()))?)
.subject(&config.subject)
.multipart(MultiPart::mixed()
.singlepart(SinglePart::plain(config.body.clone()))
.singlepart(Attachment::new(filename).body(data, mime)),
 )
.map_err(|e| MailError(e.to_string()))?;
 let creds = Credentials::new(config.sender_email.clone(), config.sender_password.clone());
 let transport = if config.use_tls {
 SmtpTransport::relay(&config.smtp_server)
.map_err(|e| MailError(e.to_string()))?
.port(config.smtp_port)
.credentials(creds)
.build()
 } else {
 SmtpTransport::builder_dangerous(&config.smtp_server)
.port(config.smtp_port)
.credentials(creds)
.build()
 };
 transport
.send(&email)
.map(|_| ())
.map_err(|e| MailError(e.to_string()))
}
