import type { Address } from 'viem';

const RAW = process.env.NEXT_PUBLIC_CONTRACT_ADDRESS ?? '';

export const CONTRACT_ADDRESS = RAW as Address;

/** Whether a real (non-zero) contract address has been configured. */
export const CONTRACT_CONFIGURED =
  /^0x[0-9a-fA-F]{40}$/.test(RAW) &&
  RAW.toLowerCase() !== '0x0000000000000000000000000000000000000000';
