use alloy_evm::precompiles::DynPrecompile;
use alloy_primitives::Address;
use arbos_revm::transaction::ArbitrumTransactionError;
use foundry_evm::core::evm::{EthEvm, EthEvmContext, TxEnv};
use revm::{
    Database, DatabaseCommit, Inspector,
    context_interface::result::{EVMError, ExecutionResult, ResultAndState},
    handler::PrecompileProvider,
    interpreter::{InterpreterResult, interpreter::EthInterpreter},
};
use std::fmt::Debug;

/// Object-safe trait that enables injecting extra precompiles when using
/// `anvil` as a library.
pub trait PrecompileFactory: Send + Sync + Unpin + Debug {
    /// Returns a set of precompiles to extend the EVM with.
    fn precompiles(&self) -> Vec<(Address, DynPrecompile)>;
}

/// A wrapper around [`EthEvm`] for anvil execution. Always runs with inspector enabled.
pub struct AnvilEvm<DB: Database, I, P>(pub EthEvm<DB, I, P>);

impl<DB: Database, I, P> AnvilEvm<DB, I, P> {
    pub fn inspector(&self) -> &I {
        &self.0.0.inspector
    }

    pub fn inspector_mut(&mut self) -> &mut I {
        &mut self.0.0.inspector
    }

    pub fn precompiles(&self) -> &P {
        &self.0.0.precompiles
    }

    pub fn precompiles_mut(&mut self) -> &mut P {
        &mut self.0.0.precompiles
    }
}

impl<DB, I, P> AnvilEvm<DB, I, P>
where
    DB: Database,
    I: Inspector<EthEvmContext<DB>, EthInterpreter>,
    P: PrecompileProvider<EthEvmContext<DB>, Output = InterpreterResult>,
{
    pub fn transact(
        &mut self,
        tx: TxEnv,
    ) -> Result<ResultAndState, EVMError<DB::Error, ArbitrumTransactionError>> {
        use revm::InspectEvm;
        self.0.inspect_tx(tx)
    }

    pub fn transact_commit(
        &mut self,
        tx: TxEnv,
    ) -> Result<ExecutionResult, EVMError<DB::Error, ArbitrumTransactionError>>
    where
        DB: DatabaseCommit,
    {
        use revm::InspectCommitEvm;
        self.0.inspect_tx_commit(tx)
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use crate::PrecompileFactory;
    use alloy_evm::precompiles::{DynPrecompile, PrecompileInput};
    use alloy_primitives::{Address, Bytes, TxKind, address};
    use arbos_revm::{ArbitrumEvm, precompiles::ArbitrumPrecompileProvider};
    use foundry_evm::core::evm::{EthEvmContext, LocalContext, PrecompilesMap, TxEnv};
    use itertools::Itertools;
    use revm::{
        Journal,
        context::{JournalTr, TxEnv as BaseTxEnv},
        database::EmptyDBTyped,
        handler::{PrecompileProvider, instructions::EthInstructions},
        inspector::NoOpInspector,
        interpreter::interpreter::EthInterpreter,
        precompile::PrecompileOutput,
        primitives::hardfork::SpecId,
    };

    use super::AnvilEvm;

    // A precompile activated in the `Prague` spec.
    const ETH_PRAGUE_PRECOMPILE: Address = address!("0x0000000000000000000000000000000000000011");

    // A custom precompile address and payload for testing.
    const PRECOMPILE_ADDR: Address = address!("0x00000000000000000000000000000000000f0071");
    const PAYLOAD: &[u8] = &[0xde, 0xad, 0xbe, 0xef];

    #[derive(Debug)]
    struct CustomPrecompileFactory;

    impl PrecompileFactory for CustomPrecompileFactory {
        fn precompiles(&self) -> Vec<(Address, DynPrecompile)> {
            vec![(
                PRECOMPILE_ADDR,
                DynPrecompile::from(|input: PrecompileInput<'_>| {
                    Ok(PrecompileOutput {
                        bytes: Bytes::copy_from_slice(input.data),
                        gas_used: 0,
                        gas_refunded: 0,
                        reverted: false,
                    })
                }),
            )]
        }
    }

    type TestCfgEnv = foundry_evm::core::evm::CfgEnv;

    /// Creates a new EVM instance with the custom precompile factory.
    #[allow(clippy::type_complexity)]
    fn create_eth_evm(
        spec: SpecId,
    ) -> (
        foundry_evm::Env,
        AnvilEvm<EmptyDBTyped<Infallible>, NoOpInspector, PrecompilesMap<EmptyDBTyped<Infallible>>>,
    ) {
        let eth_env = foundry_evm::Env {
            evm_env: foundry_evm::EvmEnv {
                block_env: Default::default(),
                cfg_env: TestCfgEnv::new_with_spec(spec),
            },
            tx: TxEnv::from(BaseTxEnv {
                kind: TxKind::Call(PRECOMPILE_ADDR),
                data: PAYLOAD.into(),
                ..Default::default()
            }),
        };

        let eth_evm_context = EthEvmContext {
            journaled_state: Journal::new(revm::database::EmptyDB::default()),
            block: eth_env.evm_env.block_env.clone(),
            cfg: eth_env.evm_env.cfg_env.clone(),
            tx: eth_env.tx.clone(),
            chain: (),
            local: LocalContext::default(),
            error: Ok(()),
        };

        let eth_evm = AnvilEvm(ArbitrumEvm::new_with_inspector(
            eth_evm_context,
            NoOpInspector,
            EthInstructions::<EthInterpreter, EthEvmContext<revm::database::EmptyDB>>::default(),
            PrecompilesMap::new(ArbitrumPrecompileProvider::new(spec)),
        ));

        (eth_env, eth_evm)
    }

    #[test]
    fn build_eth_evm_with_extra_precompiles_default_spec() {
        let (env, mut evm) = create_eth_evm(SpecId::default());

        // Check that the Prague precompile IS present when using the default spec.
        assert!(evm.precompiles().warm_addresses().contains(&ETH_PRAGUE_PRECOMPILE));

        assert!(!evm.precompiles().warm_addresses().contains(&PRECOMPILE_ADDR));

        evm.precompiles_mut().extend_precompiles(CustomPrecompileFactory.precompiles());

        assert!(evm.precompiles().warm_addresses().contains(&PRECOMPILE_ADDR));

        let result = evm.transact(env.tx).unwrap();

        assert!(result.result.is_success());
        assert_eq!(result.result.output(), Some(&PAYLOAD.into()));
    }

    #[test]
    fn build_eth_evm_with_extra_precompiles_london_spec() {
        let (env, mut evm) = create_eth_evm(SpecId::LONDON);

        // Check that the Prague precompile IS NOT present when using the London spec.
        assert!(!evm.precompiles().warm_addresses().contains(&ETH_PRAGUE_PRECOMPILE));

        assert!(!evm.precompiles().warm_addresses().contains(&PRECOMPILE_ADDR));

        evm.precompiles_mut().extend_precompiles(CustomPrecompileFactory.precompiles());

        assert!(evm.precompiles().warm_addresses().contains(&PRECOMPILE_ADDR));

        let result = evm.transact(env.tx).unwrap();

        assert!(result.result.is_success());
        assert_eq!(result.result.output(), Some(&PAYLOAD.into()));
    }
}
