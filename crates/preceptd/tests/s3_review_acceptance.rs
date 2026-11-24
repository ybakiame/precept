//! S3 consumer-visible gaps, not renamed S2 cases or native UI evidence.
//! Build workspace binaries first. Every daemon/root/Profile is owned synthetic.
//! Direct SQLite changes below are explicitly contained fixture/fault injection;
//! every subsequent process request still passes production validation.
#[allow(dead_code)] // Shared harness; this suite uses part of it.
mod support;

#[path = "s3_review_acceptance/bindings.rs"]
mod bindings;
#[path = "s3_review_acceptance/capacity.rs"]
mod capacity;
#[path = "s3_review_acceptance/concurrency.rs"]
mod concurrency;
#[path = "s3_review_acceptance/harness.rs"]
mod harness;
#[path = "s3_review_acceptance/identity.rs"]
mod identity;
#[path = "s3_review_acceptance/projection.rs"]
mod projection;
#[path = "s3_review_acceptance/queue.rs"]
mod queue;
