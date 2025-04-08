# ModernTensor CLI Guide

## Wallet Commands

### 1. Create Coldkey

**Description:** Creates a new coldkey (mnemonic) and saves it encrypted.  
**Command:**

```bash
python sdk/cli/main.py w create-coldkey --name <coldkey_name>
```

**Notes:**

- You will be prompted to enter and confirm a password
- Coldkeys are stored in the `moderntensor` directory by default

### 2. Load Coldkey

**Description:** Loads an existing coldkey into memory.  
**Command:**

```bash
python sdk/cli/main.py w load-coldkey --name <coldkey_name>
```

**Notes:**

- Requires password for decryption
- Needed before generating hotkeys

### 3. Generate Hotkey

**Description:** Creates a new hotkey (public key) for a coldkey.  
**Command:**

```bash
python sdk/cli/main.py w generate-hotkey --coldkey <coldkey_name> --hotkey-name <hotkey_name>
```

**Notes:**

- Requires coldkey password
- Hotkeys are stored in hotkeys.json

### 4. Import Hotkey

**Description:** Imports an encrypted hotkey string.  
**Command:**

```bash
python sdk/cli/main.py w import-hotkey --coldkey <coldkey_name> --encrypted-hotkey <key_string> --hotkey-name <hotkey_name>
```

**Notes:**

- Use `--overwrite` flag to replace existing hotkey

### 5. List Keys

**Description:** Displays all coldkeys and associated hotkeys in a tree structure.  
**Command:**

```bash
python sdk/cli/main.py w list
```

## Stake Commands

### 1. Initialize Wallet

**Description:** Prepares wallet for staking operations.  
**Command:**

```bash
python sdk/cli/main.py stake init-wallet --coldkey <coldkey_name> --hotkey <hotkey_name>
```

### 2. Get Balance

**Description:** Shows wallet balance and UTXOs.  
**Command:**

```bash
python sdk/cli/main.py stake get-balance --coldkey <coldkey_name> --hotkey <hotkey_name>
```

### 3. Delegate Stake

**Description:** Delegates to a stake pool.  
**Command:**

```bash
python sdk/cli/main.py stake delegate --coldkey <coldkey_name> --hotkey <hotkey_name> --pool-id <pool_id_hex>
```

### 4. Redelegate Stake

**Description:** Changes delegation to new pool.  
**Command:**

```bash
python sdk/cli/main.py stake redelegate --coldkey <coldkey_name> --hotkey <hotkey_name> --new-pool-id <new_pool_id_hex>
```

### 5. Withdraw Rewards

**Description:** Withdraws staking rewards.  
**Command:**

```bash
python sdk/cli/main.py stake withdraw-rewards --coldkey <coldkey_name> --hotkey <hotkey_name>
```
