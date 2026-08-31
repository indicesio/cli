#![allow(clippy::all)]
#![allow(missing_docs)]
#![allow(unused_qualifications)]

include!(concat!(env!("OUT_DIR"), "/indices_openapi.rs"));

impl ClientHooks<()> for Client {
    async fn pre<E>(
        &self,
        request: &mut reqwest::Request,
        _info: &OperationInfo,
    ) -> std::result::Result<(), Error<E>> {
        crate::telemetry::inject_trace_context(request.headers_mut());
        Ok(())
    }

    async fn exec(
        &self,
        request: reqwest::Request,
        _info: &OperationInfo,
    ) -> reqwest::Result<reqwest::Response> {
        let response = self.client().execute(request).await?;
        crate::client::replay_json_response(response).await
    }
}
