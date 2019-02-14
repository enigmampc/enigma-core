pub extern crate enigma_core_app as app;

extern crate zmq;
extern crate regex;
pub extern crate ethabi;
pub extern crate serde;
extern crate rmp_serde as rmps;
pub extern crate enigma_crypto;
extern crate rustc_hex as hex;
pub extern crate cross_test_utils;
extern crate futures;
extern crate dirs;
extern crate rand;
extern crate tempfile;

use self::cross_test_utils::{generate_contract_address, make_encrypted_response,
                             get_fake_state_key, get_bytecode_from_path};
use self::app::*;
use self::futures::Future;
use self::app::networking::*;
use self::serde::{Deserialize, Serialize};
use self::rmps::{Deserializer, Serializer};
use self::app::serde_json;
use app::serde_json::*;
use std::thread;
use self::regex::Regex;
use self::hex::{ToHex, FromHex};
use self::ethabi::{Token};
use self::enigma_crypto::asymmetric::KeyPair;
use self::enigma_crypto::symmetric;
use self::rand::{thread_rng, Rng};
use app::db::DB;
use self::tempfile::TempDir;

/// It's important to save TempDir too, because when it gets dropped the directory will be removed.
pub fn create_test_db() -> (DB, TempDir) {
    let tempdir = tempfile::tempdir().unwrap();
    let db = DB::new(tempdir.path(), true).unwrap();
    (db, tempdir)
}

pub fn run_core(port: &'static str) {
    thread::spawn(move || {
        let enclave = esgx::general::init_enclave_wrapper().expect("[-] Init Enclave Failed");
        let eid = enclave.geteid();

        let (mut db, _datadir) = create_test_db();
        let server = IpcListener::new(&format!("tcp://*:{}", port));
        let spid = "1601F95C39B9EA307FEAABB901ADC3EE";
        server
            .run(move |multi| ipc_listener::handle_message(&mut db, multi, spid, eid))
            .wait()
            .unwrap();

    });
}

pub fn generate_job_id() -> String {
    let mut rng = thread_rng();
    let id: u32 = rng.gen();
    id.to_string()
}

pub fn is_hex(msg: &str) -> bool {
    let re = Regex::new(r"^(0x|0X)?[0-9a-fA-F]*$").unwrap();
    re.is_match(msg)
}

pub fn conn_and_call_ipc(msg: &str, port: &'static str) -> Value {
    let context = zmq::Context::new();
    let requester = context.socket(zmq::REQ).unwrap();
    assert!(requester.connect(&format!("tcp://localhost:{}", port)).is_ok());

    requester.send(msg, 0).unwrap();

    let mut msg = zmq::Message::new();
    requester.recv(&mut msg, 0).unwrap();
    serde_json::from_str(msg.as_str().unwrap()).unwrap()
}
pub fn get_simple_msg_format(msg_type: &str) -> Value {
    json!({"id": &generate_job_id(), "type": msg_type})
}

pub fn get_msg_format_with_input(type_tip: &str, input: &str) -> Value {
    json!({"id": &generate_job_id(), "type": type_tip, "input": input})
}

pub fn get_encryption_msg(user_pubkey: [u8; 64]) -> Value {
    json!({"id" : &generate_job_id(), "type" : "NewTaskEncryptionKey", "userPubKey": user_pubkey.to_hex()})
}

pub fn get_ptt_req_msg(addrs: &[String]) -> Value {
    json!({"id" : &generate_job_id(), "type" : "GetPTTRequest", "input": addrs.to_vec()})
}

pub fn get_ptt_res_msg(response: &[u8]) -> Value {
    json!({"id" : &generate_job_id(), "type" : "PTTResponse", "response": response.to_hex()})
}

pub fn get_deploy_msg(pre_code: &str, args: &str, callable: &str, usr_pubkey: &str, gas_limit: u64, addr: &str) -> Value {
    json!({"id" : &generate_job_id(), "type" : "DeploySecretContract", "input":
            {"preCode": &pre_code, "encryptedArgs": args,
            "encryptedFn": callable, "userDHKey": usr_pubkey,
            "gasLimit": gas_limit, "contractAddress": addr}
            })
}

pub fn get_compute_msg(task_id: &str, callable: &str, args: &str, user_pubkey: &str, gas_limit: u64, con_addr: &str) -> Value {
    json!({"id": &generate_job_id(), "type": "ComputeTask", "input": { "taskID": task_id, "encryptedArgs": args,
    "encryptedFn": callable, "userDHKey": user_pubkey, "gasLimit": gas_limit, "contractAddress": con_addr}})
}

pub fn get_get_tips_msg(input: &[String]) -> Value {
    json!({"id": &generate_job_id(), "type": "GetTips", "input": input.to_vec()})
}

pub fn get_delta_msg(addr: &str, key: u64) -> Value {
    json!({"id": &generate_job_id(), "type": "GetDelta", "input": {"address": addr, "key": key}})
}

pub fn get_deltas_msg(_input: &[(String, u64, u64)]) -> Value {
    let input: Vec<Value> = _input.iter().map(|(addr, from, to)| json!({"address": addr, "from": from, "to": to})).collect();
    json!({"id": &generate_job_id(), "type": "GetDeltas", "input": input})
}

pub fn get_msg_format_update_contract(addr: &str, bytecode: &str) -> Value {
    json!({"id": &generate_job_id(), "type": "UpdateNewContract", "address": addr, "bytecode": bytecode})
}

pub fn get_update_deltas_msg(_input: &[(String, u64, String)]) -> Value {
    let input: Vec<Value> = _input.iter().map(|(addr, key, data)| json!({"address": addr, "key": key, "delta": data})).collect();
    json!({"id": &generate_job_id(), "type": "UpdateDeltas", "deltas": input})
}

#[derive(Debug)]
pub struct ParsedMessage {
    prefix: String,
    pub data: Vec<String>,
    pub pub_key: Vec<u8>,
    id: [u8; 12],
}

impl ParsedMessage {
    pub fn from_value(msg: &Value) -> Self {
        let prefix_bytes: Vec<u8> = serde_json::from_value(msg["prefix"].clone()).unwrap();
        let prefix: String = std::str::from_utf8(&prefix_bytes[..]).unwrap().to_string();

        let data_bytes: Vec<Vec<u8>> = serde_json::from_value(msg["data"]["Request"].clone()).unwrap();
        let mut data: Vec<String> = Vec::new();
        for a in data_bytes {
            data.push(a.to_hex());
        }
        let pub_key: Vec<u8> = serde_json::from_value(msg["pubkey"].clone()).unwrap();
        let id: [u8; 12] = serde_json::from_value(msg["id"].clone()).unwrap();

        Self { prefix, data, pub_key, id }
    }
}

pub fn parse_packed_msg(msg: &str) -> Value {
    let msg_bytes = msg.from_hex().unwrap();
    let mut _de = Deserializer::new(&msg_bytes[..]);
    Deserialize::deserialize(&mut Deserializer::new(&msg_bytes[..])).unwrap()
}

pub fn mock_principal_res(msg: &str) -> Vec<u8> {
    let unpacked_msg: Value = parse_packed_msg(msg);
    let enc_response: Value = make_encrypted_response(&unpacked_msg);

    let mut serialized_enc_response = Vec::new();
    enc_response.serialize(&mut Serializer::new(&mut serialized_enc_response)).unwrap();
    serialized_enc_response
}

pub fn run_ptt_round(port: &'static str, addrs: &[String]) -> Value {

    // set encrypted request message to send to the principal node
    let msg_req = get_ptt_req_msg(&addrs);
    let req_val: Value = conn_and_call_ipc(&msg_req.to_string(), port);
    let packed_msg = req_val["result"]["request"].as_str().unwrap();

    let enc_response = mock_principal_res(packed_msg);
    let msg = get_ptt_res_msg(&enc_response);
    conn_and_call_ipc(&msg.to_string(), port)
}

pub fn produce_shared_key(port: &'static str) -> ([u8; 32], [u8; 64]) {
    // get core's pubkey
    let keys = KeyPair::new().unwrap();
    let msg = get_encryption_msg(keys.get_pubkey());

    let v: Value = conn_and_call_ipc(&msg.to_string(), port);
    let core_pubkey: String = serde_json::from_value(v["result"]["workerEncryptionKey"].clone()).unwrap();
    let _pubkey_vec: Vec<u8> = core_pubkey.from_hex().unwrap();
    let mut pubkey_arr = [0u8; 64];
    pubkey_arr.copy_from_slice(&_pubkey_vec);

    let shared_key = keys.derive_key(&pubkey_arr).unwrap();
    (shared_key, keys.get_pubkey())
}

pub fn full_erc20_deployment(port: &'static str, gas_limit: Option<u64>) -> (Value, [u8; 32]) {
    // address generation and ptt
    let address = generate_contract_address();
    let _ = run_ptt_round(port, &[address.to_hex()]);

    // WUKE- get the arguments encryption key
    let (_shared_key, _user_pubkey) = produce_shared_key(port);

    let pre_code = get_bytecode_from_path("../../examples/eng_wasm_contracts/erc20");
    let fn_deploy = "construct()";
    let args_deploy = [];
    let (encrypted_callable, encrypted_args) = encrypt_args(&args_deploy, fn_deploy, _shared_key);
    let gas_limit = gas_limit.unwrap_or(100_000_000);

    let msg = get_deploy_msg(&pre_code.to_hex(), &encrypted_args.to_hex(),
                             &encrypted_callable.to_hex(), &_user_pubkey.to_hex(), gas_limit, &address.to_hex());
    let v: Value = conn_and_call_ipc(&msg.to_string(), port);

    (v, address.into())
}

pub fn erc20_deployment_without_ptt_to_addr(port: &'static str, _address: &str) -> Value {
    let (shared_key, user_pubkey) = produce_shared_key(port);

    let pre_code = get_bytecode_from_path("../../examples/eng_wasm_contracts/erc20");
    let fn_deploy = "construct()";
    let args_deploy = [];
    let (encrypted_callable, encrypted_args) = encrypt_args(&args_deploy, fn_deploy, shared_key);
    let gas_limit = 100_000_000;

    let msg = get_deploy_msg(&pre_code.to_hex(), &encrypted_args.to_hex(),
                             &encrypted_callable.to_hex(), &user_pubkey.to_hex(), gas_limit, _address);
    conn_and_call_ipc(&msg.to_string(), port)
}

pub fn full_simple_deployment(port: &'static str) -> (Value, [u8; 32]) {
    // address generation and ptt
    let address = generate_contract_address();
    let _ = run_ptt_round(port, &[address.to_hex()]);

    // WUKE- get the arguments encryption key
    let (shared_key, user_pubkey) = produce_shared_key(port);

    let pre_code = get_bytecode_from_path("../../examples/eng_wasm_contracts/simplest");
    let fn_deploy = "construct(uint)";
    let args_deploy = [Token::Uint(17.into())];
    let (encrypted_callable, encrypted_args) = encrypt_args(&args_deploy, fn_deploy, shared_key);
    let gas_limit = 100_000_000;

    let msg = get_deploy_msg(&pre_code.to_hex(), &encrypted_args.to_hex(),
                             &encrypted_callable.to_hex(), &user_pubkey.to_hex(), gas_limit, &address.to_hex());
    let v: Value = conn_and_call_ipc(&msg.to_string(), port);

    (v, address.into())
}

pub fn full_addition_compute(port: &'static str,  a: u64, b: u64) -> (Value, [u8; 32], [u8; 32]) {
    let (_, contract_addr): (_, [u8; 32]) = full_simple_deployment(port);
    let args = [Token::Uint(a.into()), Token::Uint(b.into())];
    let callable  = "addition(uint,uint)";
    let (result, key) = contract_compute(port, contract_addr, &args, callable);
    (result, key, contract_addr)
}

pub fn full_mint_compute(port: &'static str,  user_addr: &[u8; 32], amount: u64) -> (Value,  [u8;32], [u8; 32]) {
    let (_, contract_addr): (_, [u8; 32]) = full_erc20_deployment(port, None);
    let args = [Token::FixedBytes(user_addr.to_vec()), Token::Uint(amount.into())];
    let callable  = "mint(bytes32,uint256)";
    let (result, key) = contract_compute(port, contract_addr, &args, callable);
    (result, key, contract_addr)
}

pub fn contract_compute(port: &'static str,  contract_addr: [u8; 32], args: &[Token], callable: &str) -> (Value, [u8; 32]) {
    // WUKE- get the arguments encryption key
    let (shared_key, user_pubkey) = produce_shared_key(port);

    let task_id: String = generate_contract_address().to_hex();
    let (encrypted_callable, encrypted_args) = encrypt_args(args, callable, shared_key);
    let gas_limit = 100_000_000;

    let msg = get_compute_msg(&task_id, &encrypted_callable.to_hex(), &encrypted_args.to_hex(),
                              &user_pubkey.to_hex(), gas_limit, &contract_addr.to_hex());
    (conn_and_call_ipc(&msg.to_string(), port), shared_key)
}

fn encrypt_args( args:&[Token], callable: &str, key: [u8;32]) -> (Vec<u8>, Vec<u8>) {
    (symmetric::encrypt(callable.as_bytes(), &key).unwrap(),
     symmetric::encrypt(&ethabi::encode(args), &key).unwrap())
}

pub fn encrypt_addr_delta(addr: [u8; 32], delta: &[u8]) -> String {
    let state_key = get_fake_state_key(addr.into());
    let enc = symmetric::encrypt(delta, &state_key).unwrap();
    enc.to_hex()
}

pub fn decrypt_addr_delta(addr: [u8; 32], delta: &[u8]) -> Vec<u8> {
    let state_key = get_fake_state_key(addr.into());
    symmetric::decrypt(delta, &state_key).unwrap()
}

pub fn decrypt_delta_to_value(addr: [u8; 32], delta: &[u8]) -> Value {
    let dec = decrypt_addr_delta(addr, delta);
    let mut des = Deserializer::new(&dec[..]);
    Deserialize::deserialize(&mut des).unwrap()
}

pub fn decrypt_output_to_uint(output: &[u8], key: &[u8; 32]) -> Token {
    let dec = symmetric::decrypt(output, key).unwrap();
    ethabi::decode(&[ethabi::ParamType::Uint(256)], &dec).unwrap().pop().unwrap()
}

pub fn deploy_and_compute_few_contracts(port: &'static str) -> Vec<[u8; 32]> {
    let (_, _, contract_address_a): (_, _, [u8; 32]) = full_addition_compute(port, 56, 87);
    let (_, _, contract_address_b): (_, _ , [u8; 32]) = full_addition_compute(port, 75, 43);
    let (_, _, contract_address_c): (_, _, [u8; 32]) = full_mint_compute(port, &generate_contract_address().into(), 500);
    vec![contract_address_a, contract_address_b, contract_address_c]
}

pub fn send_update_contract(port: &'static str,  addr: &str, bytecode: &str) -> Value {
    let msg = get_msg_format_update_contract(addr, bytecode);
    conn_and_call_ipc(&msg.to_string(), port)
}
