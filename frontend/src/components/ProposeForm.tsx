'use client';

import { useMemo, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useAccount, useSwitchChain, useWaitForTransactionReceipt, useWriteContract } from 'wagmi';
import { BaseError } from 'viem';
import { contractAbi, DispatchTimeKind, Track } from '@/lib/abi';
import { CONTRACT_ADDRESS } from '@/lib/contract';
import { chain } from '@/lib/chain';
import { useActiveAccount } from '@/lib/activeAccount';
import { callHashFromHex, isAddress, isHash32, parseAddresses } from '@/lib/format';

type Mode = 'hash' | 'bytes';

export function ProposeForm() {
  const { isConnected, chainId } = useAccount();
  const { activeAddress: address } = useActiveAccount();
  const { switchChain, isPending: switching } = useSwitchChain();
  const queryClient = useQueryClient();

  // The wallet's actual chain — independent of our wagmi config. If this isn't
  // Paseo Hub, signing would land the TX on the wrong network (e.g. mainnet).
  const wrongChain = isConnected && chainId !== chain.id;

  const [mode, setMode] = useState<Mode>('hash');
  const [callHash, setCallHash] = useState('');
  const [callLen, setCallLen] = useState('');
  const [callBytes, setCallBytes] = useState('');
  const [enactmentKind, setEnactmentKind] = useState<DispatchTimeKind>(DispatchTimeKind.After);
  const [enactmentBlock, setEnactmentBlock] = useState('100');
  const [track, setTrack] = useState<Track>(Track.WhitelistedCaller);
  const [approversRaw, setApproversRaw] = useState('');
  const [minApprovers, setMinApprovers] = useState('1');

  const { writeContract, data: hash, isPending, error: writeError, reset } = useWriteContract();
  const { isLoading: confirming, isSuccess } = useWaitForTransactionReceipt({ hash });

  // Derive callHash/callLen from pasted SCALE call bytes (blake2-256).
  const derived = useMemo(() => {
    if (mode !== 'bytes' || !callBytes.trim()) return null;
    try {
      return { ...callHashFromHex(callBytes), error: null as string | null };
    } catch (e) {
      return { callHash: '' as `0x${string}`, callLen: 0, error: (e as Error).message };
    }
  }, [mode, callBytes]);

  const effHash = mode === 'bytes' ? derived?.callHash ?? '' : callHash.trim();
  const effLen = mode === 'bytes' ? derived?.callLen ?? 0 : Number(callLen);

  // Normalise approvers: dedupe + sort ascending (the contract requires the set
  // to be strictly increasing by raw address bytes).
  const approvers = useMemo(() => {
    const list = parseAddresses(approversRaw).filter(isAddress);
    const uniq = Array.from(new Map(list.map((a) => [a.toLowerCase(), a])).values());
    return uniq.sort((a, b) => (a.toLowerCase() < b.toLowerCase() ? -1 : 1));
  }, [approversRaw]);

  const creatorIsApprover =
    !!address && approvers.some((a) => a.toLowerCase() === address.toLowerCase());

  const minN = Number(minApprovers);
  const blockN = Number(enactmentBlock);

  const problems: string[] = [];
  if (mode === 'hash' && callHash && !isHash32(effHash)) problems.push('Call hash must be 32 bytes (0x + 64 hex).');
  if (mode === 'bytes' && derived?.error) problems.push(derived.error);
  if (effLen <= 0) problems.push('Call length must be > 0.');
  if (approvers.length === 0) problems.push('Add at least one approver.');
  if (creatorIsApprover) problems.push('Your own address cannot be an approver.');
  if (!Number.isInteger(minN) || minN < 1) problems.push('Min approvers must be ≥ 1.');
  if (approvers.length > 0 && minN > approvers.length) problems.push('Min approvers exceeds approver count.');
  if (!Number.isInteger(blockN) || blockN < 0) {
    problems.push(
      enactmentKind === DispatchTimeKind.At
        ? 'Enactment block must be ≥ 0.'
        : 'Enactment delay must be ≥ 0.',
    );
  }

  const ready =
    isConnected && !wrongChain && isHash32(effHash) && effLen > 0 && approvers.length > 0 && !creatorIsApprover && problems.length === 0;

  function submit() {
    reset();
    writeContract(
      {
        // Sign as the account the user picked in the connect modal, not just the
        // wallet's primary account.
        account: address,
        // Pin the target chain: wagmi asserts the wallet is on Paseo Hub and
        // throws ChainMismatchError rather than silently signing on whatever
        // network the wallet happens to be set to.
        chainId: chain.id,
        address: CONTRACT_ADDRESS,
        abi: contractAbi,
        functionName: 'propose',
        args: [
          effHash as `0x${string}`,
          effLen,
          { kind: enactmentKind, block: blockN },
          track,
          approvers,
          BigInt(minN),
        ],
      },
      {
        onSuccess: () => queryClient.invalidateQueries(),
      },
    );
  }

  return (
    <div className="panel rise">
      <div className="seg">
        <button className={mode === 'hash' ? 'active' : ''} onClick={() => setMode('hash')}>
          I have a preimage hash
        </button>
        <button className={mode === 'bytes' ? 'active' : ''} onClick={() => setMode('bytes')}>
          I have the call bytes
        </button>
      </div>

      {mode === 'hash' ? (
        <div className="grid-2">
          <div className="field">
            <label>Call hash (bytes32)</label>
            <input
              className={`input ${callHash && !isHash32(effHash) ? 'invalid' : ''}`}
              placeholder="0x…"
              value={callHash}
              onChange={(e) => setCallHash(e.target.value)}
            />
          </div>
          <div className="field">
            <label>Call length (bytes)</label>
            <input
              className="input"
              inputMode="numeric"
              placeholder="e.g. 42"
              value={callLen}
              onChange={(e) => setCallLen(e.target.value.replace(/[^\d]/g, ''))}
            />
          </div>
        </div>
      ) : (
        <div className="field">
          <label>SCALE-encoded call (hex)</label>
          <textarea
            className={`textarea ${derived?.error ? 'invalid' : ''}`}
            placeholder="0x… the runtime call you want enacted"
            value={callBytes}
            onChange={(e) => setCallBytes(e.target.value)}
          />
          <div className="hint">
            blake2-256 →{' '}
            <span className="mono">{derived?.callHash || '—'}</span>
            {derived && !derived.error ? ` · ${derived.callLen} bytes` : ''}
          </div>
        </div>
      )}

      <div className="grid-2">
        <div className="field">
          <label>Enactment</label>
          <div className="row">
            <select
              className="input"
              value={enactmentKind}
              onChange={(e) => setEnactmentKind(Number(e.target.value) as DispatchTimeKind)}
            >
              <option value={DispatchTimeKind.After}>After (delay)</option>
              <option value={DispatchTimeKind.At}>At (block)</option>
            </select>
            <input
              className="input"
              inputMode="numeric"
              placeholder={enactmentKind === DispatchTimeKind.At ? 'block number' : 'blocks to wait'}
              value={enactmentBlock}
              onChange={(e) => setEnactmentBlock(e.target.value.replace(/[^\d]/g, ''))}
            />
          </div>
        </div>
        <div className="field">
          <label>Track</label>
          <select
            className="input"
            value={track}
            onChange={(e) => setTrack(Number(e.target.value) as Track)}
          >
            <option value={Track.WhitelistedCaller}>Whitelisted caller</option>
            <option value={Track.Root}>Root</option>
          </select>
        </div>
      </div>

      <div className="field">
        <label>Min approvers</label>
        <input
          className="input"
          inputMode="numeric"
          value={minApprovers}
          onChange={(e) => setMinApprovers(e.target.value.replace(/[^\d]/g, ''))}
        />
      </div>

      <div className="field">
        <label>Approvers ({approvers.length})</label>
        <textarea
          className="textarea"
          placeholder="One address per line. Sorted & de-duplicated automatically."
          value={approversRaw}
          onChange={(e) => setApproversRaw(e.target.value)}
        />
        {approvers.length > 0 && (
          <div className="hint mono">{approvers.join('  ·  ')}</div>
        )}
      </div>

      <div className="row spread">
        <span className="muted mono" style={{ fontSize: 11.5 }}>
          Threshold {Math.min(minN || 0, approvers.length)} / {approvers.length}
        </span>
        <button className="btn btn-primary" disabled={!ready || isPending || confirming} onClick={submit}>
          {isPending ? 'Confirm in wallet…' : confirming ? 'Submitting…' : 'Create proposal'}
        </button>
      </div>

      {!isConnected && <div className="notice warn">Connect your wallet to create a proposal.</div>}
      {wrongChain && (
        <div className="notice err row spread">
          <span>Wrong network — your wallet is not on {chain.name}. Submitting here would send the TX to the wrong chain.</span>
          <button
            className="btn"
            disabled={switching}
            onClick={() => switchChain({ chainId: chain.id })}
          >
            {switching ? 'Switching…' : `Switch to ${chain.name}`}
          </button>
        </div>
      )}
      {isConnected && !wrongChain && problems.length > 0 && (approversRaw || callHash || callBytes) && (
        <div className="notice warn">{problems[0]}</div>
      )}
      {isSuccess && <div className="notice ok">Proposal created. It will appear in the ledger below.</div>}
      {writeError && (
        <div className="notice err">
          {(writeError as BaseError).shortMessage ?? writeError.message}
        </div>
      )}
    </div>
  );
}
