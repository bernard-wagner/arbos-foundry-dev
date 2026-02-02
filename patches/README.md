# arbos-foundry Patches

This document describes the patches applied to upstream Foundry to create arbos-foundry, a fork optimized for Arbitrum development with Stylus support.

## Patch Overview

arbos-foundry is based on Foundry v1.5.1 with two patches applied:

1. **CI changes, binary renaming, alias binaries**
2. **Integrate arbos-revm, vendor alloy-evm, add Stylus support**

---

## Patch 1: CI changes, binary renaming, alias binaries

### Summary
Renames binaries from `forge`, `cast`, `anvil`, `chisel` to `arbos-forge`, `arbos-cast`, `arbos-anvil`, `arbos-chisel` while maintaining backward-compatible alias binaries.

### Files Modified

#### CI Workflows (`.github/workflows/`)
- `ci.yml` - Updated for arbos-foundry naming
- `docs.yml` - Updated references
- `release.yml` - Release configuration for arbos binaries
- `release-debug.yml` - New debug release workflow
- `test.yml`, `test-isolate.yml` - Test configuration updates

#### Build Scripts
- `.github/scripts/matrices.py` - Build matrix for arbos binaries
- `foundryup/foundryup` - Updated installer for arbos-foundry
- `npm/` - All npm package configurations updated

#### Binary Configuration (`crates/*/Cargo.toml`)
Each binary crate has:
```toml
# Disable auto-discovery since we have multiple binaries
autobins = false

[[bin]]
name = "arbos-forge"  # Main binary with new name
path = "bin/main.rs"

[[bin]]
name = "forge"  # Alias for backward compatibility
path = "bin/forge_alias.rs"
```

#### Alias Binary Files
New files added:
- `crates/anvil/bin/anvil_alias.rs`
- `crates/cast/bin/cast_alias.rs`
- `crates/chisel/bin/chisel_alias.rs`
- `crates/forge/bin/forge_alias.rs`

Each alias is a simple wrapper:
```rust
// Alias binary for backwards compatibility and tests.
use forge::main;
fn main() { main::main() }
```

### Merge Conflict Resolution
When merging upstream Foundry updates:
1. Keep `autobins = false` and both `[[bin]]` entries
2. Preserve alias binary files unchanged
3. Update CI workflows carefully - our CI differs significantly
4. npm packages should retain arbos-foundry naming

---

## Patch 2: Integrate arbos-revm, vendor alloy-evm, add Stylus support

### Summary
This comprehensive patch:
- Removes Optimism and Celo L2 support
- Vendors a customized alloy-evm crate
- Integrates arbos-revm as the EVM backend for Arbitrum state handling
- Adds Stylus WASM contract configuration and cheatcodes

### Components

#### 2.1: Remove Optimism/Celo Support

**Files Modified:**
- `crates/anvil/Cargo.toml` - Remove op-revm, alloy-op-evm, op-alloy-* deps
- `crates/anvil/src/hardfork.rs` - Remove OpHardfork enum variant
- `crates/anvil/src/cmd.rs` - Remove Optimism hardfork parsing
- `crates/cast/Cargo.toml` - Remove op-alloy-flz, op-alloy-consensus
- `crates/cast/src/lib.rs` - Use TxEnvelope instead of OpTxEnvelope
- `crates/cast/src/cmd/da_estimate.rs` - Simplify DA estimation

**Files Deleted:**
- `crates/evm/networks/src/celo/` - Entire directory removed

**Key Changes:**
- `NetworkConfigs` simplified to empty struct with stub methods
- All `is_optimism()` checks removed
- OpHardfork conversion code removed from hardfork.rs

#### 2.2: Vendor alloy-evm Crate

**New Directory:** `crates/alloy-evm/`
- `Cargo.toml` - Dependencies for EVM types
- `src/lib.rs` - Re-exports and module organization
- `src/precompiles.rs` - PrecompilesMap implementation
- `src/overrides.rs` - State override handling
- `src/traits.rs` - EVM context traits
- `src/tx.rs` - Transaction types

**Purpose:**
The vendored alloy-evm provides:
- `EthEvmContext` type alias for arbos-revm context
- `PrecompilesMap` for precompile management
- Custom type exports needed by arbos-revm integration

**Workspace Integration:**
```toml
# Cargo.toml (workspace root)
[workspace.dependencies]
alloy-evm = { path = "crates/alloy-evm", ... }
```

#### 2.3: Wire arbos-revm as EVM Backend

**Dependency Changes:**
```toml
# Multiple Cargo.toml files
arbos-revm.workspace = true
revm = { workspace = true, features = ["optional_eip3541", ...] }
```

**Key Integration Points:**

1. **Backend State Overrides** (`crates/anvil/src/eth/backend/mem/mod.rs`):
```rust
pub async fn apply_arbitrum_state_overrides<F>(&self, f: F)
where
    F: FnOnce(&mut ArbosStateParams),
{
    // Uses actual block/cfg env instead of defaults
    // Only writes if params changed
    // Preserves Merkle proofs when no changes needed
}
```

2. **Executor Integration** (`crates/evm/evm/src/executors/mod.rs`):
```rust
pub fn apply_arbitrum_state_overrides<F>(&mut self, mut f: F)
where
    F: FnMut(&mut ArbosStateParams),
{
    let block_env = self.env.evm_env.block_env.clone();
    let cfg_env = self.env.evm_env.cfg_env.clone();
    // Apply overrides using actual environment
}
```

3. **EVM Context** (`crates/evm/core/src/evm.rs`):
- Uses `EthEvmContext` from vendored alloy-evm
- Integrates with arbos-revm state handling

**Merkle Proof Preservation:**
The implementation carefully avoids unnecessary state modifications:
```rust
if !self.stylus_config.is_default() {
    // Only modify state if config differs from defaults
    backend.apply_arbitrum_state_overrides(...).await;
}
```

#### 2.4: Add Stylus Configuration and Cheatcodes

**Configuration Files:**
- `crates/config/src/stylus.rs` - StylusConfig struct
- `crates/config/src/lib.rs` - Re-export and integration
- `crates/cli/src/opts/evm.rs` - CLI arguments for Stylus

**StylusConfig Structure:**
```rust
pub struct StylusConfig {
    /// Address of the StylusDeployer contract
    pub deployer_address: Option<Address>,
    // Additional Arbitrum state parameters
}
```

**Cheatcode Files:**
- `crates/cheatcodes/src/stylus.rs` - Stylus cheatcode implementations
- `crates/cheatcodes/spec/src/vm.rs` - Cheatcode definitions
- `crates/cheatcodes/assets/cheatcodes.json` - JSON schema

**Cheatcodes Added:**
```solidity
// Deploy Stylus WASM contracts
function deployStylusCode(string calldata artifactPath) returns (address);
function deployStylusCode(string calldata, bytes calldata args) returns (address);
// ... additional overloads with value and salt

// Get compressed Stylus bytecode
function getStylusCode(string calldata artifactPath) returns (bytes memory);

// Brotli compression utilities
function brotliCompress(bytes calldata data) returns (bytes memory);
function brotliDecompress(bytes calldata compressed) returns (bytes memory);
```

**Test Fixtures:**
- `testdata/fixtures/Stylus/foundry_stylus_program.wasm`
- `testdata/fixtures/Stylus/foundry_stylus_program.wat`
- `testdata/default/cheats/Brotli.t.sol`
- `testdata/default/cheats/GetStylusCode.t.sol`

### Merge Conflict Resolution

When merging upstream Foundry updates:

1. **Dependency Conflicts**: Keep arbos-revm, remove op-revm/op-alloy-*
2. **alloy-evm**: Our vendored version may need updates if upstream alloy-evm changes significantly
3. **Backend Changes**: Preserve apply_arbitrum_state_overrides methods
4. **Config Changes**: Preserve StylusConfig integration
5. **Cheatcodes**: Preserve stylus.rs module and cheatcode definitions
6. **Test Changes**: Preserve Stylus test fixtures

### Testing

After merging upstream changes:
```bash
cargo check --workspace
cargo clippy --workspace
cargo test --workspace
```

Pay special attention to:
- EVM execution tests (may need arbos-revm-specific handling)
- Cheatcode tests (stylus cheatcodes require fixtures)
- Fork tests (ensure state handling works correctly)

---

## Applying Patches to New Foundry Releases

1. **Create branch from new upstream release:**
   ```bash
   git checkout v1.6.0  # New upstream version
   git checkout -b patches/verified-v1.6.0
   ```

2. **Cherry-pick patches:**
   ```bash
   git cherry-pick <patch1-sha>
   git cherry-pick <patch2-sha>
   ```

3. **Resolve conflicts following guidelines above**

4. **Verify:**
   ```bash
   cargo check --workspace
   cargo clippy --workspace
   cargo test --workspace
   ```

5. **Update this README if patch structure changes**
