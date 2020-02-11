use rustc_hex::ToHex;
use sgx_types::{sgx_enclave_id_t, sgx_status_t};
use web3::types::{Bytes, U256};

use enigma_tools_m::keeper_types::InputWorkerParams;
use enigma_types::{EnclaveReturn, traits::SliceCPtr};

use common_u::custom_errors::EnclaveError;
use epochs::epoch_types::{encode, SignedEpoch};

extern "C" {
    fn ecall_set_worker_params(
        eid: sgx_enclave_id_t, retval: &mut EnclaveReturn, worker_params_rlp: *const u8, worker_params_rlp_len: usize,
        seed_in: &[u8; 32], nonce_in: &[u8; 32],
        rand_out: &mut [u8; 32], nonce_out: &mut [u8; 32], sig_out: &mut [u8; 65],
    ) -> sgx_status_t;
}

/// Returns an EpochState object containing the 32 bytes signed random seed and an incremented account nonce.
/// If the `epoch_state` param is some, verify the corresponding sealed `Epoch` marker
/// Otherwise, create a new `Epoch`
///
/// # Arguments
/// * `eid` - The Enclave Id
/// * `worker_params` - The `InputWorkerParams` to store in an `Epoch`
/// * `epoch_state` - Optional, the existing `EpochState` to verify against sealed `Epoch` marker
///
/// # Examples
/// ```
/// let enclave = esgx::general::init_enclave().unwrap();
/// let result = self.contract.get_active_workers(block_number)?;
/// let worker_params: InputWorkerParams = InputWorkerParams { block_number, workers: result.0, stakes: result.1 };
/// let sig = set_worker_params(enclave.geteid(), worker_params, None).unwrap();
/// ```
#[logfn(DEBUG)]
pub fn set_or_verify_worker_params(eid: sgx_enclave_id_t, worker_params: &InputWorkerParams, epoch: Option<SignedEpoch>) -> Result<SignedEpoch, EnclaveError> {
    let mut retval: EnclaveReturn = EnclaveReturn::Success;
    let (nonce_in, seed_in) = match epoch.clone() {
        Some(e) => (e.get_nonce().into(), e.get_seed().into()),
        None => ([0; 32], [0; 32])
    };
    debug!("Calling enclave set_worker_params with nonce/seed: {:?}/{:?}", nonce_in.to_vec().to_hex(), seed_in.to_vec().to_hex());
    let (mut nonce_out, mut rand_out) = ([0; 32], [0; 32]);
    let mut sig_out: [u8; 65] = [0; 65];
    // Serialize the InputWorkerParams into RLP
    let worker_params_rlp = encode(worker_params);
    let status = unsafe {
        ecall_set_worker_params(
            eid,
            &mut retval,
            worker_params_rlp.as_c_ptr() as *const u8,
            worker_params_rlp.len(),
            &seed_in,
            &nonce_in,
            &mut rand_out,
            &mut nonce_out,
            &mut sig_out,
        )
    };
    if retval != EnclaveReturn::Success || status != sgx_status_t::SGX_SUCCESS {
        return Err(EnclaveError::Failure { err: retval, status });
    }
    // If an `Epoch` was given and the ecall succeeded, it is considered verified
    // Otherwise, build a new `Epoch` from the parameters of the new epoch
    let epoch_out = match epoch {
        Some(epoch) => epoch,
        None => {
            let seed = U256::from_big_endian(&rand_out);
            let sig = Bytes(sig_out.to_vec());
            let nonce = U256::from_big_endian(&nonce_out);
            SignedEpoch::new(seed, sig, nonce, worker_params.km_block_number)
        }
    };
    Ok(epoch_out)
}

#[cfg(test)]
pub mod tests {
    use web3::types::H160;

    use esgx::general::init_enclave_wrapper;

    use super::*;

    pub fn get_worker_params(km_block_number: u64, workers: Vec<[u8; 20]>, stakes: Vec<u64>) -> InputWorkerParams {
        InputWorkerParams {
            km_block_number: U256::from(km_block_number),
            workers: workers.into_iter().map(|a| H160(a)).collect::<Vec<H160>>(),
            stakes: stakes.into_iter().map(|s| U256::from(s)).collect::<Vec<U256>>(),
        }
    }

    //TODO: Test error scenario with `set_mock_worker_params`

    #[test]
    fn test_set_mock_worker_params() {
        let enclave = init_enclave_wrapper().unwrap();
        let workers: Vec<[u8; 20]> = vec![
            [156, 26, 193, 252, 165, 167, 191, 244, 251, 126, 53, 154, 158, 14, 64, 194, 164, 48, 231, 179],
        ];
        let stakes: Vec<u64> = vec![90000000000];
        let km_block_number = 1;
        let worker_params = get_worker_params(km_block_number, workers, stakes);
        let epoch_state = set_or_verify_worker_params(enclave.geteid(), &worker_params, None).unwrap();
        assert!(epoch_state.confirmed_state.is_none());
        enclave.destroy();
    }

    #[test]
    fn test_set_mock_worker_params_above_cap() {
        let enclave = init_enclave_wrapper().unwrap();
        let workers: Vec<[u8; 20]> = vec![
            [156, 26, 193, 252, 165, 167, 191, 244, 251, 126, 53, 154, 158, 14, 64, 194, 164, 48, 231, 179],
        ];
        let stakes: Vec<u64> = vec![90000000000];
        let km_block_number = 1;
        let worker_params = get_worker_params(km_block_number, workers, stakes);
        for _ in 0..5 {
            let epoch_state = set_or_verify_worker_params(enclave.geteid(), &worker_params, None).unwrap();
            assert!(epoch_state.confirmed_state.is_none());
        }
        enclave.destroy();
    }
}
