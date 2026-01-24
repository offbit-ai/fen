use std::num::NonZeroU32;
use std::sync::Arc;
use std::task::{Context, Poll};

use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use axum::response::IntoResponse;
use governor::clock::DefaultClock;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter};
use tower::{Layer, Service};

/// Rate limiter type using governor
type GlobalRateLimiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

/// Rate limiting layer for axum
#[derive(Clone)]
pub struct RateLimitLayer {
    limiter: Arc<GlobalRateLimiter>,
}

impl RateLimitLayer {
    /// Create a new rate limiting layer
    ///
    /// # Arguments
    /// * `requests_per_second` - Maximum requests per second
    /// * `burst_size` - Maximum burst capacity
    pub fn new(requests_per_second: u32, burst_size: u32) -> Self {
        let quota =
            Quota::per_second(NonZeroU32::new(requests_per_second).unwrap_or(NonZeroU32::MIN))
                .allow_burst(NonZeroU32::new(burst_size).unwrap_or(NonZeroU32::MIN));

        let limiter = Arc::new(RateLimiter::direct(quota));

        tracing::info!(
            rps = %requests_per_second,
            burst = %burst_size,
            "Rate limiter initialized"
        );

        Self { limiter }
    }
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitService {
            inner,
            limiter: self.limiter.clone(),
        }
    }
}

/// Rate limiting service wrapper
#[derive(Clone)]
pub struct RateLimitService<S> {
    inner: S,
    limiter: Arc<GlobalRateLimiter>,
}

impl<S> Service<Request<Body>> for RateLimitService<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let limiter = self.limiter.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Check rate limit
            match limiter.check() {
                Ok(_) => {
                    // Request allowed, proceed
                    inner.call(req).await
                }
                Err(_) => {
                    // Rate limit exceeded
                    tracing::warn!("Rate limit exceeded");

                    let response = (
                        StatusCode::TOO_MANY_REQUESTS,
                        [("Retry-After", "1")],
                        "Rate limit exceeded. Please try again later.",
                    )
                        .into_response();

                    Ok(response.map(Body::new))
                }
            }
        })
    }
}

/// Keyed rate limiter by IP address for more granular control
#[allow(dead_code)]
pub mod keyed {
    use std::net::IpAddr;

    use super::*;
    use governor::state::keyed::DefaultKeyedStateStore;

    /// IP-keyed rate limiter
    type KeyedRateLimiter = RateLimiter<IpAddr, DefaultKeyedStateStore<IpAddr>, DefaultClock>;

    /// Keyed rate limiting layer (by IP address)
    #[derive(Clone)]
    pub struct KeyedRateLimitLayer {
        limiter: Arc<KeyedRateLimiter>,
    }

    impl KeyedRateLimitLayer {
        /// Create a new keyed rate limiting layer
        pub fn new(requests_per_second: u32, burst_size: u32) -> Self {
            let quota =
                Quota::per_second(NonZeroU32::new(requests_per_second).unwrap_or(NonZeroU32::MIN))
                    .allow_burst(NonZeroU32::new(burst_size).unwrap_or(NonZeroU32::MIN));

            let limiter = Arc::new(RateLimiter::keyed(quota));

            tracing::info!(
                rps = %requests_per_second,
                burst = %burst_size,
                "Keyed rate limiter initialized (per-IP)"
            );

            Self { limiter }
        }

        /// Get the underlying limiter for use in extractors
        pub fn limiter(&self) -> Arc<KeyedRateLimiter> {
            self.limiter.clone()
        }
    }
}
