use ethabi::Token;
use ethereum_types::{H256, H160, U256};
use sgx_trts::trts::rsgx_read_rand;
use sgx_types::*;
use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::path;
use std::str;
use std::string::ToString;
use std::sync::SgxMutex;
use std::sync::SgxMutexGuard;

use enigma_crypto::hash::Keccak256;
use enigma_tools_t::common::errors_t::{EnclaveError, EnclaveError::*, EnclaveSystemError::*};
use enigma_tools_t::common::EthereumAddress;
use enigma_tools_t::common::ToHex;
use enigma_tools_t::common::utils_t::LockExpectMutex;
use enigma_tools_t::document_storage_t::{is_document, load_sealed_document, save_sealed_document, SEAL_LOG_SIZE, SealedDocumentStorage};
use epoch_keeper_t::epoch_t::{Epoch, EpochNonce};
use keys_keeper_t::keeper_types_t::{decode, InputWorkerParams, RawEncodable};
use ocalls_t;

use crate::SIGNING_KEY;

pub mod epoch_t;

const INIT_NONCE: uint32_t = 0;
const EPOCH_DIR: &str = "epoch";

// The epoch seed contains the seeds + a nonce that must match the Ethereum tx
lazy_static! { pub static ref EPOCH: SgxMutex< HashMap<U256, Epoch >> = SgxMutex::new(HashMap::new()); }

/// The epoch root path is guaranteed to exist of the enclave was initialized
fn get_epoch_root_path() -> path::PathBuf {
    let mut path_buf = ocalls_t::get_home_path().unwrap();
    path_buf.push(EPOCH_DIR);
    path_buf
}

fn get_epoch_nonce_path() -> path::PathBuf {
    get_epoch_root_path().join("nonce.sealed")
}

fn get_epoch(epoch_map: &HashMap<U256, Epoch, RandomState>, block_number: Option<U256>) -> Result<Option<Epoch>, EnclaveError> {
    println!("Getting epoch for block number: {:?}", block_number);
    if block_number.is_some() {
        return Err(SystemError(WorkerAuthError { err: "Epoch lookup by block number not implemented.".to_string() }));
    }
    if epoch_map.is_empty() {
        println!("Epoch not found");
        let nonce_path = get_epoch_nonce_path();
        if is_document(&nonce_path) {
            println!("Unsealing epoch nonce");
            let mut sealed_log_out = [0u8; SEAL_LOG_SIZE];
            load_sealed_document(&nonce_path, &mut sealed_log_out)?;
            let doc = SealedDocumentStorage::<EpochNonce>::unseal(&mut sealed_log_out)?;
            match doc {
                Some(doc) => {
                    let nonce = Some(doc.data);
                    println!("found epoch marker: {:?}", nonce);
                    //TODO: unseal the epoch
                }
                None => ()
            }
        }
        return Ok(None);
    }
    // The epoch map cannot be empty here
    let nonce = epoch_map.keys().max().unwrap().clone();
    let epoch: Epoch = epoch_map.get(&nonce).unwrap().clone();
    Ok(Some(epoch))
}

/// Creates new epoch both in the cache and as sealed documents
fn new_epoch(nonce_map: &mut HashMap<U256, Epoch, RandomState>, worker_params: &InputWorkerParams, nonce: U256, seed: U256) -> Result<Epoch, EnclaveError> {
    let mut marker_doc: SealedDocumentStorage<EpochNonce> = SealedDocumentStorage {
        version: 0x1234, //TODO: what's this?
        data: [0; 32],
    };
    marker_doc.data = nonce.into();
    let mut sealed_log_in = [0u8; SEAL_LOG_SIZE];
    marker_doc.seal(&mut sealed_log_in)?;
    // Save sealed_log to file
    let marker_path = get_epoch_nonce_path();
    save_sealed_document(&marker_path, &sealed_log_in)?;
    println!("Sealed the epoch marker: {:?}", marker_path);

    let epoch = Epoch {
        nonce,
        seed,
        worker_params: worker_params.clone(),
    };
    //TODO: seal the epoch
    println!("Storing epoch: {:?}", epoch);
    match nonce_map.insert(nonce, epoch.clone()) {
        Some(prev) => println!("New epoch stored successfully, previous epoch: {:?}", prev),
        None => println!("Initial epoch stored successfully"),
    }
    Ok(epoch)
}

pub(crate) fn ecall_set_worker_params_internal(worker_params_rlp: &[u8], rand_out: &mut [u8; 32],
                                               nonce_out: &mut [u8; 32], sig_out: &mut [u8; 65]) -> Result<(), EnclaveError> {
    // RLP decoding the necessary data
    let worker_params = decode(worker_params_rlp);
    println!("Successfully decoded RLP worker parameters");
    let mut guard = EPOCH.lock_expect("Epoch");

    let nonce: U256 = get_epoch(&*guard, None)?
        .map_or_else(|| INIT_NONCE.into(), |_| guard.keys().max().unwrap() + 1);

    println!("Generated a nonce by incrementing the previous by 1 {:?}", nonce);
    *nonce_out = EpochNonce::from(nonce);

    rsgx_read_rand(&mut rand_out[..])?;

    let seed = U256::from(rand_out.as_ref());
    println!("Generated random seed: {:?}", seed);
    let epoch = new_epoch(&mut guard, &worker_params, nonce, seed)?;

    let msg = epoch.raw_encode()?;
    *sig_out = SIGNING_KEY.sign(&msg)?;
    println!("Signed the message : 0x{}", msg.to_hex());
    Ok(())
}

pub(crate) fn ecall_get_epoch_worker_internal(sc_addr: H256, block_number: Option<U256>) -> Result<H160, EnclaveError> {
    let guard = EPOCH.lock_expect("Epoch");
    let epoch = match get_epoch(&guard, block_number)? {
        Some(epoch) => epoch,
        None => {
            return Err(SystemError(WorkerAuthError { err: format!("No epoch found for block number (None == latest): {:?}", block_number) }));
        }
    };
    println!("Running worker selection using Epoch: {:?}", epoch);
    let worker = epoch.get_selected_worker(sc_addr)?;
    Ok(worker)
}

pub mod tests {
    use ethereum_types::{H160, U256};

    use super::*;

    //noinspection RsTypeCheck
    pub fn test_get_epoch_worker_internal() {
        let worker_params = InputWorkerParams {
            block_number: U256::from(1),
            workers: vec![H160::from(0), H160::from(1), H160::from(2), H160::from(3)],
            stakes: vec![U256::from(1), U256::from(1), U256::from(1), U256::from(1)],
        };
        let epoch = Epoch {
            nonce: U256::from(0),
            seed: U256::from(1),
            worker_params,
        };
        println!("The epoch: {:?}", epoch);
        let sc_addr = H256::from(1);
        let worker = epoch.get_selected_worker(sc_addr).unwrap();
        println!("The selected workers: {:?}", worker);
    }
}