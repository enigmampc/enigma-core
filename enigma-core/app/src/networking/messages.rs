use serde_json;
use zmq::Message;
use crate::db::{Delta, Stype, DeltaKey};
use hex::ToHex;
use failure::Error;

type Status = i8;
pub const FAILED: Status = -1;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IpcMessageRequest {
    pub id: String,
    #[serde(flatten)]
    pub request: IpcRequest
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IpcMessageResponse {
    pub id: String,
    #[serde(flatten)]
    pub response: IpcResponse
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum IpcResponse {
    GetRegistrationParams { #[serde(flatten)] result: IpcResults },
    GetTip { result: IpcDelta },
    GetTips { result: IpcResults },
    GetAllTips { result: IpcResults },
    GetAllAddrs { result: IpcResults },
    GetDelta { result: IpcResults },
    GetDeltas { result: IpcResults },
    GetContract { result: IpcResults },
    UpdateNewContract { address: String, result: IpcResults },
    UpdateDeltas { #[serde(flatten)] result: IpcResults },
    NewTaskEncryptionKey { #[serde(flatten)] result: IpcResults },
    DeploySecretContract { #[serde(flatten)] result: IpcResults},
    ComputeTask { #[serde(flatten)] result: IpcResults },
    GetPTTRequest { #[serde(flatten)] result: IpcResults },
    PTTResponse { result: IpcResults },
    Error { msg: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", rename = "result")]
pub enum IpcResults {
    Errors(Vec<IpcStatusResult>),
    #[serde(rename = "result")]
    Request { request: String, #[serde(rename = "workerSig")] sig: String },
    Addresses(Vec<String>),
    Delta(String),
    Deltas(Vec<IpcDelta>),
    Bytecode(String),
    Status(Status),
    Tips(Vec<IpcDelta>),
    #[serde(rename = "result")]
    UpdateDeltasResult { status: Status, errors: Vec<IpcStatusResult> },
    #[serde(rename = "result")]
    DHKey { #[serde(rename = "workerEncryptionKey")] dh_key: String, #[serde(rename = "workerSig")] sig: String },
    #[serde(rename = "result")]
    RegistrationParams { #[serde(rename = "signingKey")] signing_key: String, report: String, signature: String },
    #[serde(rename = "result")]
    ComputeResult {
        #[serde(rename = "usedGas")]
        used_gas: u64,
        output: String,
        delta: IpcDelta,
        signature: String,
    },
    #[serde(rename = "result")]
    DeployResult {
        #[serde(rename = "preCodeHash")]
        pre_code_hash: String,
        #[serde(rename = "usedGas")]
        used_gas: u64,
        output: String,
        delta: IpcDelta,
        signature: String,
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum IpcRequest {
    GetRegistrationParams,
    GetTip { input: String },
    GetTips { input: Vec<String> },
    GetAllTips,
    GetAllAddrs,
    GetDelta { input: IpcDelta },
    GetDeltas { input: Vec<IpcGetDeltas> },
    GetContract { input: String },
    UpdateNewContract { address: String, bytecode: String },
    UpdateDeltas { deltas: Vec<IpcDelta> },
    NewTaskEncryptionKey { #[serde(rename = "userPubKey")] user_pubkey: String },
    DeploySecretContract { input: IpcTask},
    ComputeTask { input: IpcTask },
    GetPTTRequest { input: Addresses },
    PTTResponse {  input: PrincipalResponse },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IpcTask {
    #[serde(rename = "preCode")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_code: Option<String>,
    #[serde(rename = "encryptedArgs")]
    pub encrypted_args: String,
    #[serde(rename = "encryptedFn")]
    pub encrypted_fn: String,
    #[serde(rename = "userDHKey")]
    pub user_dhkey: String,
    #[serde(rename = "gasLimit")]
    pub gas_limit: u64,
    #[serde(rename = "contractAddress")]
    pub address: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IpcStatusResult {
    pub address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<u32>,
    pub status: Status,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct IpcDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    pub key: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IpcGetDeltas {
    pub address: String,
    pub from: u32,
    pub to: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename = "response")]
pub struct PrincipalResponse (pub String);

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Addresses (pub Vec<String>);

impl std::ops::Deref for Addresses {
    type Target = Vec<String>;
    fn deref(&self) -> &Vec<String> {
        &self.0
    }
}

impl IpcMessageResponse {
    pub fn from_response(response: IpcResponse, id: String) -> Self {
        Self { id, response }
    }
}
impl IpcMessageRequest {
    pub fn from_request(request: IpcRequest, id: String) -> Self {
        Self { id, request }
    }
}



impl IpcDelta {
    pub fn from_delta_key(k: DeltaKey, v: &[u8]) -> Result<Self, Error> {
        if let Stype::Delta(indx) = k.key_type {
            Ok( IpcDelta { address: Some(k.contract_address.to_hex()), key: indx, delta: Some(v.to_hex()) } )
        } else {
            bail!("This isn't a delta")
        }
    }
}

impl From<Delta> for IpcDelta {
    fn from(delta: Delta) -> Self {
        let address = delta.key.contract_address.to_hex();
        let value = delta.value.to_hex();
        let key = delta.key.key_type.unwrap_delta();

        IpcDelta { address: Some(address), key, delta: Some(value) }
    }
}

impl From<Message> for IpcMessageRequest {
    fn from(msg: Message) -> Self {
        let msg_str = msg.as_str().unwrap();
        let req: Self = serde_json::from_str(msg_str).expect(msg_str);
        req
    }
}

impl Into<Message> for IpcMessageResponse {
    fn into(self) -> Message {
        let msg = serde_json::to_vec(&self).unwrap();
        Message::from_slice(&msg)
    }
}

pub(crate) trait UnwrapError<T> {
    fn unwrap_or_error(self) -> T;
}

impl<E: std::fmt::Debug> UnwrapError<IpcResponse> for Result<IpcResponse, E> {
    fn unwrap_or_error(self) -> IpcResponse {
        match self {
            Ok(m) => m,
            Err(e) => {
                error!("Unwrapped p2p Message failed: {:?}", e);
                IpcResponse::Error {msg: format!("{:?}", e)}
            }
        }
    }
}
