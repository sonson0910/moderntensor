# Moderntensor CLI Guide

## Wallet Commands

### 1. Create Coldkey

Creates a new coldkey (mnemonic) and saves it encrypted.

```bash
mtcli w create-coldkey --name <coldkey_name> --base-dir "moderntensor"
```

**Notes:**

- You will be prompted to enter and confirm a password
- Coldkeys are stored in the `moderntensor` directory by default

### 2. Load Coldkey

Loads an existing coldkey into memory.

```bash
mtcli w load-coldkey --name <coldkey_name> --base-dir "moderntensor"
```

**Notes:**

- Requires password for decryption
- Needed before generating hotkeys

### 3. Generate Hotkey

Creates a new hotkey (public key) for a coldkey.

```bash
mtcli w generate-hotkey --coldkey <coldkey_name> --hotkey-name <hotkey_name> --base-dir "moderntensor"
```

**Notes:**

- Requires coldkey password
- Hotkeys are stored in hotkeys.json

### 4. Import Hotkey

Imports an encrypted hotkey string.

```bash
mtcli w import-hotkey --coldkey <coldkey_name> --encrypted-hotkey <key_string> --hotkey-name <hotkey_name> --base-dir "moderntensor"
```

**Notes:**

- Use `--overwrite` flag to replace existing hotkey

### 5. List Keys

Displays all coldkeys and associated hotkeys in a tree structure.

```bash
mtcli w list
```

## Stake Commands

### 1. Initialize Wallet

Prepares wallet for staking operations.

```bash
mtcli stake init-wallet --coldkey <coldkey_name> --hotkey <hotkey_name>
```

### 2. Get Balance

Shows wallet balance and UTXOs.

```bash
mtcli stake get-balance --coldkey <coldkey_name> --hotkey <hotkey_name>
```

### 3. Delegate Stake

Delegates to a stake pool.

```bash
mtcli stake delegate --coldkey <coldkey_name> --hotkey <hotkey_name> --pool-id <pool_id_hex>
```

### 4. Redelegate Stake

Changes delegation to new pool.

```bash
mtcli stake redelegate --coldkey <coldkey_name> --hotkey <hotkey_name> --new-pool-id <new_pool_id_hex>
```

### 5. Withdraw Rewards

Withdraws staking rewards.

```bash
mtcli stake withdraw-rewards --coldkey <coldkey_name> --hotkey <hotkey_name>
```
