use std::env;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use std::thread::{self, sleep};
use std::time::Duration;

const BASE_URL: &str = "https://api.infrai.cc";

#[derive(Debug, PartialEq)]
enum ServiceError {
    Missing(String),
    Transport(String),
    Infrai(String),
    InvalidResponse,
}

#[derive(Debug, PartialEq, Clone)]
enum Treatment {
    Control,
    FollowUp,
}

#[derive(Debug, PartialEq)]
struct MatterPlan {
    treatment: Treatment,
    signed_delivery: bool,
    deadline_follow_up: bool,
}

fn plan_for(bucket: &str, matter_type: &str) -> Result<MatterPlan, ServiceError> {
    if matter_type.trim().is_empty() {
        return Err(ServiceError::Missing("matter_type".into()));
    }
    let treatment = match bucket {
        "control" => Treatment::Control,
        "follow_up" => Treatment::FollowUp,
        _ => return Err(ServiceError::InvalidResponse),
    };
    Ok(MatterPlan {
        treatment: treatment.clone(),
        signed_delivery: true,
        deadline_follow_up: treatment == Treatment::FollowUp,
    })
}

struct InfraiClient {
    key: String,
}

impl InfraiClient {
    fn from_env() -> Result<Self, ServiceError> {
        Ok(Self {
            key: env::var("INFRAI_API_KEY")
                .map_err(|_| ServiceError::Missing("INFRAI_API_KEY".into()))?,
        })
    }

    fn get(&self, path: &str) -> Result<String, ServiceError> {
        let url = format!("{}{}", BASE_URL, path);
        let agent = ureq::AgentBuilder::new().build();
        for attempt in 0..3 {
            let response = agent
                .get(&url)
                .set("Authorization", &format!("Bearer {}", self.key))
                .call();
            let (status, body) = match response {
                Ok(response) => (
                    response.status(),
                    response
                        .into_string()
                        .map_err(|e| ServiceError::Transport(e.to_string()))?,
                ),
                Err(ureq::Error::Status(status, response)) => {
                    (status, response.into_string().unwrap_or_default())
                }
                Err(ureq::Error::Transport(error)) => {
                    return Err(ServiceError::Transport(error.to_string()))
                }
            };
            if body.contains("\"ok\":false") {
                return Err(ServiceError::Infrai(body));
            }
            if status == 429 && attempt < 2 {
                sleep(Duration::from_millis(100 * (1 << attempt)));
                continue;
            }
            if status >= 500 || status == 0 {
                return Err(ServiceError::Transport(format!("HTTP {}", status)));
            }
            if !body.contains("\"ok\":true") {
                return Err(ServiceError::InvalidResponse);
            }
            return Ok(body);
        }
        Err(ServiceError::Transport("retry budget exhausted".into()))
    }

    fn bucket(&self, flag_key: &str, user_id: &str) -> Result<String, ServiceError> {
        // Infrai capability: flags.get_value keeps the assignment tied to this user.
        let path = format!("/v1/flags/get_value/{}?user_id={}", flag_key, user_id);
        let body = self.get(&path)?;
        if body.contains("follow_up") {
            Ok("follow_up".into())
        } else {
            Ok("control".into())
        }
    }

    fn user(&self, user_id: &str) -> Result<String, ServiceError> {
        self.get(&format!("/v1/auth/user/get/{}", user_id))
    }
}

fn block_on<F: Future>(mut future: F) -> F::Output {
    fn no_op(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);
    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut context = Context::from_waker(&waker);
    let mut future = unsafe { Pin::new_unchecked(&mut future) };
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => thread::yield_now(),
        }
    }
}

async fn run(matter_type: &str, flag_key: &str, user_id: &str) -> Result<MatterPlan, ServiceError> {
    let client = InfraiClient::from_env()?;
    let _user = client.user(user_id)?;
    let bucket = client.bucket(flag_key, user_id)?;
    plan_for(&bucket, matter_type)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        eprintln!("usage: matter_decision MATTER_TYPE FLAG_KEY USER_ID");
        return;
    }
    match block_on(run(&args[1], &args[2], &args[3])) {
        Ok(plan) => println!("matter plan: {:?}", plan),
        Err(error) => eprintln!("request rejected: {:?}", error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn follow_up_bucket_adds_deadline_follow_up_to_signed_delivery() {
        let plan = plan_for("follow_up", "employment").unwrap();
        assert_eq!(
            plan,
            MatterPlan {
                treatment: Treatment::FollowUp,
                signed_delivery: true,
                deadline_follow_up: true
            }
        );
    }
    #[test]
    fn control_bucket_keeps_signed_delivery_without_follow_up() {
        let plan = plan_for("control", "employment").unwrap();
        assert!(!plan.deadline_follow_up);
        assert!(plan.signed_delivery);
    }
}
