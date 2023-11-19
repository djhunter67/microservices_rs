use std::sync::Mutex;

use crate::{sessions::Sessions, users::Users};

// use tonic::codegen::http::status;
use tonic::{Request, Response, Status};

use authentication::auth_server::Auth;
use authentication::{
    SignInRequest, SignInResponse, SignOutRequest, SignOutResponse, SignUpRequest, SignUpResponse,
    StatusCode,
};

pub mod authentication {
    tonic::include_proto!("authentication");
}

// Re-exporting
pub use authentication::auth_server::AuthServer;
pub use tonic::transport::Server;

pub struct AuthService {
    users_service: Box<Mutex<dyn Users + Send + Sync>>,
    sessions_service: Box<Mutex<dyn Sessions + Send + Sync>>,
}

impl AuthService {
    pub fn new(
        users_service: Box<Mutex<dyn Users + Send + Sync>>,
        sessions_service: Box<Mutex<dyn Sessions + Send + Sync>>,
    ) -> Self {
        Self {
            users_service,
            sessions_service,
        }
    }
}

#[tonic::async_trait]
impl Auth for AuthService {
    async fn sign_in(
        &self,
        request: Request<SignInRequest>,
    ) -> Result<Response<SignInResponse>, Status> {
        println!("Got a request: {:?}", request);

        let req = request.into_inner();

        // Get user's uuid from `users_service`. Panic if the lock is poisoned.
        let user_uuid: Option<String> = match self.users_service.lock() {
            Ok(users_service) => users_service,
            Err(_) => panic!("Poisoned lock"),
        }
        .get_user_uuid(req.username.clone(), req.password);

        // Match on `result`. If `result` is `None` return a SignInResponse with a the `status_code` set to `Failure`
        let mut sigin = SignInResponse {
            status_code: StatusCode::Success.into(),
            session_token: "".to_owned(),
            user_uuid: "".to_owned(),
        };

        let user_uuid = match user_uuid {
            None => {
                let reply = SignInResponse {
                    status_code: StatusCode::Failure.into(),
                    session_token: "".to_owned(),
                    user_uuid: "".to_owned(),
                };
                return Ok(Response::new(reply));
            }
            Some(uuid) => uuid,
        };

        // and `user_uuid`/`session_token` set to empty strings.

        // Create new session using `sessions_service`. Panic if the lock is poisoned.
        let session_token = match self.sessions_service.lock() {
            Ok(sessions_service) => sessions_service,
            Err(_) => panic!("Poisoned lock"),
        }
        .create_session(&user_uuid);

        sigin.session_token = session_token;
        sigin.user_uuid = user_uuid;
        sigin.status_code = StatusCode::Success.into();

        println!("USER signin: {:?}", sigin);

        Ok(Response::new(sigin))
    }

    async fn sign_up(
        &self,
        request: Request<SignUpRequest>,
    ) -> Result<Response<SignUpResponse>, Status> {
        println!("Got a request: {:?}", request);

        let req = request.into_inner();

        // Create a new user through `users_service`. Panic if the lock is poisoned.
        let result: Result<(), String> = match self.users_service.is_poisoned() {
            true => panic!("Poisoned lock"),
            false => self.users_service.lock().unwrap(),
        }
        .create_user(req.username.clone(), req.password);

        // TODO: Return a `SignUpResponse` with the appropriate `status_code` based on `result`.
        match result {
            Ok(_) => {
                let result = SignUpResponse {
                    status_code: StatusCode::Success.into(),
                };
                return Ok(Response::new(result));
            }
            Err(_) => {
                let result = SignUpResponse {
                    status_code: StatusCode::Failure.into(),
                };
                return Ok(Response::new(result));
            }
        }
    }

    async fn sign_out(
        &self,
        request: Request<SignOutRequest>,
    ) -> Result<Response<SignOutResponse>, Status> {
        println!("Got a request: {:?}", request);

        let req = request.into_inner();

        // TODO: Delete session using `sessions_service`.
        match self.sessions_service.is_poisoned() {
            true => panic!("Poisoned lock"),
            false => self.sessions_service.lock(),
        }
        .expect("Unable to lock")
        .delete_session(&req.session_token);
        
	// Create `SignOutResponse` with `status_code` set to `Success`
        let reply: SignOutResponse = SignOutResponse {
            status_code: StatusCode::Success.into(),
        };
        Ok(Response::new(reply))
    }
}

#[cfg(test)]
mod tests {
    use crate::{sessions::SessionsImpl, users::UsersImpl};

    use super::*;

    #[tokio::test]
    async fn sign_in_should_fail_if_user_not_found() {
        let users_service = Box::new(Mutex::new(UsersImpl::default()));
        let sessions_service = Box::new(Mutex::new(SessionsImpl::default()));

        let auth_service = AuthService::new(users_service, sessions_service);

        let request = tonic::Request::new(SignInRequest {
            username: "123456".to_owned(),
            password: "654321".to_owned(),
        });

        let result = auth_service.sign_in(request).await.unwrap().into_inner();

        assert_eq!(result.status_code, StatusCode::Failure.into());
        assert_eq!(result.user_uuid.is_empty(), true);
        assert_eq!(result.session_token.is_empty(), true);
    }

    #[tokio::test]
    async fn sign_in_should_fail_if_incorrect_password() {
        let mut users_service = UsersImpl::default();

        let _ = users_service.create_user("123456".to_owned(), "654321".to_owned());

        let users_service = Box::new(Mutex::new(users_service));
        let sessions_service = Box::new(Mutex::new(SessionsImpl::default()));

        let auth_service = AuthService::new(users_service, sessions_service);

        let request = tonic::Request::new(SignInRequest {
            username: "123456".to_owned(),
            password: "wrong password".to_owned(),
        });

        let result = auth_service.sign_in(request).await.unwrap().into_inner();

        assert_eq!(result.status_code, StatusCode::Failure.into());
        assert_eq!(result.user_uuid.is_empty(), true);
        assert_eq!(result.session_token.is_empty(), true);
    }

    #[tokio::test]
    async fn sign_in_should_succeed() {
        let mut users_service = UsersImpl::default();

        let _ = users_service.create_user("123456".to_owned(), "654321".to_owned());

        let users_service = Box::new(Mutex::new(users_service));
        let sessions_service = Box::new(Mutex::new(SessionsImpl::default()));

        let auth_service = AuthService::new(users_service, sessions_service);

        let request = tonic::Request::new(SignInRequest {
            username: "123456".to_owned(),
            password: "654321".to_owned(),
        });

        let result = auth_service.sign_in(request).await.unwrap().into_inner();

        assert_eq!(result.status_code, StatusCode::Success.into());
        assert_eq!(result.user_uuid.is_empty(), false);
        assert_eq!(result.session_token.is_empty(), false);
    }

    #[tokio::test]
    async fn sign_up_should_fail_if_username_exists() {
        let mut users_service = UsersImpl::default();

        let _ = users_service.create_user("123456".to_owned(), "654321".to_owned());

        let users_service = Box::new(Mutex::new(users_service));
        let sessions_service = Box::new(Mutex::new(SessionsImpl::default()));

        let auth_service = AuthService::new(users_service, sessions_service);

        let request = tonic::Request::new(SignUpRequest {
            username: "123456".to_owned(),
            password: "654321".to_owned(),
        });

        let result = auth_service.sign_up(request).await.unwrap();

        assert_eq!(result.into_inner().status_code, StatusCode::Failure.into());
    }

    #[tokio::test]
    async fn sign_up_should_succeed() {
        let users_service = Box::new(Mutex::new(UsersImpl::default()));
        let sessions_service = Box::new(Mutex::new(SessionsImpl::default()));

        let auth_service = AuthService::new(users_service, sessions_service);

        let request = tonic::Request::new(SignUpRequest {
            username: "123456".to_owned(),
            password: "654321".to_owned(),
        });

        let result = auth_service.sign_up(request).await.unwrap();

        assert_eq!(result.into_inner().status_code, StatusCode::Success.into());
    }

    #[tokio::test]
    async fn sign_out_should_succeed() {
        let users_service = Box::new(Mutex::new(UsersImpl::default()));
        let sessions_service = Box::new(Mutex::new(SessionsImpl::default()));

        let auth_service = AuthService::new(users_service, sessions_service);

        let request = tonic::Request::new(SignOutRequest {
            session_token: "".to_owned(),
        });

        let result = auth_service.sign_out(request).await.unwrap();

        assert_eq!(result.into_inner().status_code, StatusCode::Success.into());
    }
}
