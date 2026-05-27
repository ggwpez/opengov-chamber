import { parseEther } from 'viem';

/**
 * Value that `finalize()` must be sent. The contract requires
 * `SUBMISSION_DEPOSIT (10 DOT, 10-decimal plancks) * NATIVE_TO_ETH_RATIO (1e8)`
 * which equals 1e19 — i.e. 10 units in the 18-decimal EVM view. See
 * `contract/src/contract.rs` and the project memory on the submission deposit.
 */
export const FINALIZE_DEPOSIT = parseEther('10');
