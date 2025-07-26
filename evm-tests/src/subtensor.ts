import * as assert from "assert";
import { devnet, MultiAddress } from '@polkadot-api/descriptors';
import { TypedApi, TxCallData } from 'polkadot-api';
import { KeyPair } from "@polkadot-labs/hdkd-helpers"
import { getAliceSigner, waitForTransactionCompletion, getSignerFromKeypair, waitForTransactionWithRetry } from './substrate'
import { convertH160ToSS58, convertPublicKeyToSs58, ethAddressToH160 } from './address-utils'
import { tao } from './balance-math'
import internal from "stream";

// create a new subnet and return netuid 
export async function addNewSubnetwork(api: TypedApi<typeof devnet>, hotkey: KeyPair, coldkey: KeyPair) {
    const alice = getAliceSigner()
    const totalNetworks = await api.query.SubtensorModule.TotalNetworks.getValue()

    const rateLimit = await api.query.SubtensorModule.NetworkRateLimit.getValue()
    if (rateLimit !== BigInt(0)) {
        const internalCall = api.tx.AdminUtils.sudo_set_network_rate_limit({ rate_limit: BigInt(0) })
        const tx = api.tx.Sudo.sudo({ call: internalCall.decodedCall })
        await waitForTransactionWithRetry(api, tx, alice)
    }

    const signer = getSignerFromKeypair(coldkey)
    const registerNetworkTx = api.tx.SubtensorModule.register_network({ hotkey: convertPublicKeyToSs58(hotkey.publicKey) })
    await waitForTransactionWithRetry(api, registerNetworkTx, signer)

    const newTotalNetworks = await api.query.SubtensorModule.TotalNetworks.getValue()
    // could create multiple subnetworks during retry, just return the first created one
    assert.ok(newTotalNetworks > totalNetworks)
    return totalNetworks
}

// force set balance for a ss58 address
export async function forceSetBalanceToSs58Address(api: TypedApi<typeof devnet>, ss58Address: string) {
    const alice = getAliceSigner()
    const balance = tao(1e8)
    const internalCall = api.tx.Balances.force_set_balance({ who: MultiAddress.Id(ss58Address), new_free: balance })
    const tx = api.tx.Sudo.sudo({ call: internalCall.decodedCall })

    await waitForTransactionWithRetry(api, tx, alice)

    const balanceOnChain = (await api.query.System.Account.getValue(ss58Address)).data.free
    // check the balance except for sudo account becasue of tx fee
    if (ss58Address !== convertPublicKeyToSs58(alice.publicKey)) {
        assert.equal(balance, balanceOnChain)
    }
}

// set balance for an eth address
export async function forceSetBalanceToEthAddress(api: TypedApi<typeof devnet>, ethAddress: string) {
    const ss58Address = convertH160ToSS58(ethAddress)
    await forceSetBalanceToSs58Address(api, ss58Address)
}

export async function setCommitRevealWeightsEnabled(api: TypedApi<typeof devnet>, netuid: number, enabled: boolean) {
    const value = await api.query.SubtensorModule.CommitRevealWeightsEnabled.getValue(netuid)
    if (value === enabled) {
        return;
    }

    const alice = getAliceSigner()
    const internalCall = api.tx.AdminUtils.sudo_set_commit_reveal_weights_enabled({ netuid: netuid, enabled: enabled })
    const tx = api.tx.Sudo.sudo({ call: internalCall.decodedCall })

    await waitForTransactionWithRetry(api, tx, alice)
    assert.equal(enabled, await api.query.SubtensorModule.CommitRevealWeightsEnabled.getValue(netuid))
}

export async function setWeightsSetRateLimit(api: TypedApi<typeof devnet>, netuid: number, rateLimit: bigint) {
    const value = await api.query.SubtensorModule.WeightsSetRateLimit.getValue(netuid)
    if (value === rateLimit) {
        return;
    }

    const alice = getAliceSigner()
    const internalCall = api.tx.AdminUtils.sudo_set_weights_set_rate_limit({ netuid: netuid, weights_set_rate_limit: rateLimit })
    const tx = api.tx.Sudo.sudo({ call: internalCall.decodedCall })

    await waitForTransactionWithRetry(api, tx, alice)
    assert.equal(rateLimit, await api.query.SubtensorModule.WeightsSetRateLimit.getValue(netuid))
}

// tempo is u16 in rust, but we just number in js. so value should be less than u16::Max
export async function setTempo(api: TypedApi<typeof devnet>, netuid: number, tempo: number) {
    const value = await api.query.SubtensorModule.Tempo.getValue(netuid)
    console.log("init avlue is ", value)
    if (value === tempo) {
        return;
    }

    const alice = getAliceSigner()
    const internalCall = api.tx.AdminUtils.sudo_set_tempo({ netuid: netuid, tempo: tempo })
    const tx = api.tx.Sudo.sudo({ call: internalCall.decodedCall })

    await waitForTransactionWithRetry(api, tx, alice)
    assert.equal(tempo, await api.query.SubtensorModule.Tempo.getValue(netuid))
}

export async function setCommitRevealWeightsInterval(api: TypedApi<typeof devnet>, netuid: number, interval: bigint) {
    const value = await api.query.SubtensorModule.RevealPeriodEpochs.getValue(netuid)
    if (value === interval) {
        return;
    }

    const alice = getAliceSigner()
    const internalCall = api.tx.AdminUtils.sudo_set_commit_reveal_weights_interval({ netuid: netuid, interval: interval })
    const tx = api.tx.Sudo.sudo({ call: internalCall.decodedCall })

    await waitForTransactionWithRetry(api, tx, alice)
    assert.equal(interval, await api.query.SubtensorModule.RevealPeriodEpochs.getValue(netuid))
}


export async function forceSetChainID(api: TypedApi<typeof devnet>, chainId: bigint) {
    const value = await api.query.EVMChainId.ChainId.getValue()
    if (value === chainId) {
        return;
    }

    const alice = getAliceSigner()
    const internalCall = api.tx.AdminUtils.sudo_set_evm_chain_id({ chain_id: chainId })
    const tx = api.tx.Sudo.sudo({ call: internalCall.decodedCall })

    await waitForTransactionWithRetry(api, tx, alice)
    assert.equal(chainId, await api.query.EVMChainId.ChainId.getValue())
}

export async function disableWhiteListCheck(api: TypedApi<typeof devnet>, disabled: boolean) {
    const value = await api.query.EVM.DisableWhitelistCheck.getValue()
    if (value === disabled) {
        return;
    }

    const alice = getAliceSigner()
    const internalCall = api.tx.EVM.disable_whitelist({ disabled: disabled })
    const tx = api.tx.Sudo.sudo({ call: internalCall.decodedCall })

    await waitForTransactionWithRetry(api, tx, alice)
    assert.equal(disabled, await api.query.EVM.DisableWhitelistCheck.getValue())
}

export async function burnedRegister(api: TypedApi<typeof devnet>, netuid: number, ss58Address: string, keypair: KeyPair) {
    const registered = await api.query.SubtensorModule.Uids.getValue(netuid, ss58Address);
    // just return if already registered
    if (registered !== undefined) {
        console.log("hotkey ", ss58Address, " already registered in netuid ", netuid)
        return;
    }

    await new Promise((resolve) => setTimeout(resolve, 1000));
    const uids = await api.query.SubtensorModule.SubnetworkN.getValue(netuid)
    const signer = getSignerFromKeypair(keypair)
    const tx = api.tx.SubtensorModule.burned_register({ hotkey: ss58Address, netuid: netuid })
    await waitForTransactionWithRetry(api, tx, signer)
    assert.equal(uids + 1, await api.query.SubtensorModule.SubnetworkN.getValue(netuid))
}


export async function sendProxyCall(api: TypedApi<typeof devnet>, calldata: TxCallData, ss58Address: string, keypair: KeyPair) {
    const signer = getSignerFromKeypair(keypair)
    const tx = api.tx.Proxy.proxy({
        call: calldata,
        real: MultiAddress.Id(ss58Address),
        force_proxy_type: undefined
    });
    await waitForTransactionWithRetry(api, tx, signer)
}


export async function setTxRateLimit(api: TypedApi<typeof devnet>, txRateLimit: bigint) {
    const value = await api.query.SubtensorModule.TxRateLimit.getValue()
    if (value === txRateLimit) {
        return;
    }
    const alice = getAliceSigner()

    const internalCall = api.tx.AdminUtils.sudo_set_tx_rate_limit({ tx_rate_limit: txRateLimit })
    const tx = api.tx.Sudo.sudo({ call: internalCall.decodedCall })


    await waitForTransactionWithRetry(api, tx, alice)
}

export async function setMaxAllowedValidators(api: TypedApi<typeof devnet>, netuid: number, maxAllowedValidators: number) {
    const value = await api.query.SubtensorModule.MaxAllowedValidators.getValue(netuid)
    if (value === maxAllowedValidators) {
        return;
    }

    const alice = getAliceSigner()

    const internalCall = api.tx.AdminUtils.sudo_set_max_allowed_validators({
        netuid: netuid,
        max_allowed_validators: maxAllowedValidators
    })
    const tx = api.tx.Sudo.sudo({ call: internalCall.decodedCall })

    await waitForTransactionWithRetry(api, tx, alice)
    assert.equal(maxAllowedValidators, await api.query.SubtensorModule.MaxAllowedValidators.getValue(netuid))
}

export async function setSubnetOwnerCut(api: TypedApi<typeof devnet>, subnetOwnerCut: number) {
    const value = await api.query.SubtensorModule.SubnetOwnerCut.getValue()
    if (value === subnetOwnerCut) {
        return;
    }

    const alice = getAliceSigner()

    const internalCall = api.tx.AdminUtils.sudo_set_subnet_owner_cut({
        subnet_owner_cut: subnetOwnerCut
    })
    const tx = api.tx.Sudo.sudo({ call: internalCall.decodedCall })

    await waitForTransactionWithRetry(api, tx, alice)
    assert.equal(subnetOwnerCut, await api.query.SubtensorModule.SubnetOwnerCut.getValue())
}

export async function setActivityCutoff(api: TypedApi<typeof devnet>, netuid: number, activityCutoff: number) {
    const value = await api.query.SubtensorModule.ActivityCutoff.getValue(netuid)
    if (value === activityCutoff) {
        return;
    }

    const alice = getAliceSigner()

    const internalCall = api.tx.AdminUtils.sudo_set_activity_cutoff({
        netuid: netuid,
        activity_cutoff: activityCutoff
    })
    const tx = api.tx.Sudo.sudo({ call: internalCall.decodedCall })

    await waitForTransactionWithRetry(api, tx, alice)
    assert.equal(activityCutoff, await api.query.SubtensorModule.ActivityCutoff.getValue(netuid))
}

export async function setMaxAllowedUids(api: TypedApi<typeof devnet>, netuid: number, maxAllowedUids: number) {
    const value = await api.query.SubtensorModule.MaxAllowedUids.getValue(netuid)
    if (value === maxAllowedUids) {
        return;
    }

    const alice = getAliceSigner()

    const internalCall = api.tx.AdminUtils.sudo_set_max_allowed_uids({
        netuid: netuid,
        max_allowed_uids: maxAllowedUids
    })
    const tx = api.tx.Sudo.sudo({ call: internalCall.decodedCall })

    await waitForTransactionWithRetry(api, tx, alice)
    assert.equal(maxAllowedUids, await api.query.SubtensorModule.MaxAllowedUids.getValue(netuid))
}

export async function setMinDelegateTake(api: TypedApi<typeof devnet>, minDelegateTake: number) {
    const value = await api.query.SubtensorModule.MinDelegateTake.getValue()
    if (value === minDelegateTake) {
        return;
    }

    const alice = getAliceSigner()

    const internalCall = api.tx.AdminUtils.sudo_set_min_delegate_take({
        take: minDelegateTake
    })
    const tx = api.tx.Sudo.sudo({ call: internalCall.decodedCall })

    await waitForTransactionWithRetry(api, tx, alice)
    assert.equal(minDelegateTake, await api.query.SubtensorModule.MinDelegateTake.getValue())
}

export async function becomeDelegate(api: TypedApi<typeof devnet>, ss58Address: string, keypair: KeyPair) {
    const signer = getSignerFromKeypair(keypair)

    const tx = api.tx.SubtensorModule.become_delegate({
        hotkey: ss58Address
    })
    await waitForTransactionWithRetry(api, tx, signer)
}

export async function addStake(api: TypedApi<typeof devnet>, netuid: number, ss58Address: string, amount_staked: bigint, keypair: KeyPair) {
    const signer = getSignerFromKeypair(keypair)
    let tx = api.tx.SubtensorModule.add_stake({
        netuid: netuid,
        hotkey: ss58Address,
        amount_staked: amount_staked
    })

    await waitForTransactionWithRetry(api, tx, signer)
}

export async function setWeight(api: TypedApi<typeof devnet>, netuid: number, dests: number[], weights: number[], version_key: bigint, keypair: KeyPair) {
    const signer = getSignerFromKeypair(keypair)
    let tx = api.tx.SubtensorModule.set_weights({
        netuid: netuid,
        dests: dests,
        weights: weights,
        version_key: version_key
    })

    await waitForTransactionWithRetry(api, tx, signer)
}

export async function rootRegister(api: TypedApi<typeof devnet>, ss58Address: string, keypair: KeyPair) {
    const signer = getSignerFromKeypair(keypair)
    let tx = api.tx.SubtensorModule.root_register({
        hotkey: ss58Address
    })

    await waitForTransactionWithRetry(api, tx, signer)
}

export async function setSubtokenEnable(api: TypedApi<typeof devnet>, netuid: number, subtokenEnable: boolean) {
    const signer = getAliceSigner()
    let internalTx = api.tx.AdminUtils.sudo_set_subtoken_enabled({
        netuid: netuid,
        subtoken_enabled: subtokenEnable
    })
    let tx = api.tx.Sudo.sudo({ call: internalTx.decodedCall })

    await waitForTransactionWithRetry(api, tx, signer)
}

export async function startCall(api: TypedApi<typeof devnet>, netuid: number, keypair: KeyPair) {
    const registerBlock = Number(await api.query.SubtensorModule.NetworkRegisteredAt.getValue(netuid))
    let currentBlock = await api.query.System.Number.getValue()
    const duration = Number(await api.constants.SubtensorModule.DurationOfStartCall)

    while (currentBlock - registerBlock <= duration) {
        await new Promise((resolve) => setTimeout(resolve, 2000));
        currentBlock = await api.query.System.Number.getValue()
    }
    // wait for chain to run coinbase
    await new Promise((resolve) => setTimeout(resolve, 2000));

    const signer = getSignerFromKeypair(keypair)
    let tx = api.tx.SubtensorModule.start_call({
        netuid: netuid,
    })

    await waitForTransactionWithRetry(api, tx, signer)

    await new Promise((resolve) => setTimeout(resolve, 1000));
    const callStarted = await api.query.SubtensorModule.FirstEmissionBlockNumber
        .getValue(netuid);
    assert.notEqual(callStarted, undefined);
}

export async function setMaxChildkeyTake(api: TypedApi<typeof devnet>, take: number) {
    const alice = getAliceSigner()
    const internalCall = api.tx.SubtensorModule.sudo_set_max_childkey_take({ take })
    const tx = api.tx.Sudo.sudo({ call: internalCall.decodedCall })

    await waitForTransactionWithRetry(api, tx, alice)
}

// Swap coldkey to contract address
export async function swapColdkey(
    api: TypedApi<typeof devnet>,
    coldkey: KeyPair,
    contractAddress: string,
) {
    const alice = getAliceSigner();
    const internal_tx = api.tx.SubtensorModule.swap_coldkey({
        old_coldkey: convertPublicKeyToSs58(coldkey.publicKey),
        new_coldkey: convertH160ToSS58(contractAddress),
        swap_cost: tao(10),
    });
    const tx = api.tx.Sudo.sudo({
        call: internal_tx.decodedCall,
    });
    await waitForTransactionWithRetry(api, tx, alice);
}

// Set target registrations per interval to 1000
export async function setTargetRegistrationsPerInterval(
    api: TypedApi<typeof devnet>,
    netuid: number,
) {
    const alice = getAliceSigner();
    const internal_tx = api.tx.AdminUtils
        .sudo_set_target_registrations_per_interval({
            netuid,
            target_registrations_per_interval: 1000,
        });
    const tx = api.tx.Sudo.sudo({
        call: internal_tx.decodedCall,
    });
    await waitForTransactionWithRetry(api, tx, alice);
}