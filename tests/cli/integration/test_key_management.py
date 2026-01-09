"""
Integration tests for key management operations.

Tests the key generation, derivation, and storage functionality.
"""

import pytest
import os
import tempfile
import shutil
from pathlib import Path
from sdk.keymanager.key_generator import KeyGenerator
from sdk.keymanager.encryption import encrypt_data, decrypt_data


class TestKeyGeneration:
    """Test key generation functionality."""

    def setup_method(self):
        """Setup test fixtures."""
        self.key_gen = KeyGenerator()

    def test_generate_mnemonic_12_words(self):
        """Test generating 12-word mnemonic."""
        mnemonic = self.key_gen.generate_mnemonic(words=12)
        words = mnemonic.split()
        assert len(words) == 12
        # Verify all words are non-empty
        assert all(word for word in words)

    def test_generate_mnemonic_24_words(self):
        """Test generating 24-word mnemonic."""
        mnemonic = self.key_gen.generate_mnemonic(words=24)
        words = mnemonic.split()
        assert len(words) == 24
        assert all(word for word in words)

    def test_mnemonic_uniqueness(self):
        """Test that generated mnemonics are unique."""
        mnemonic1 = self.key_gen.generate_mnemonic()
        mnemonic2 = self.key_gen.generate_mnemonic()
        assert mnemonic1 != mnemonic2

    def test_derive_hotkey_from_mnemonic(self):
        """Test deriving hotkey from mnemonic."""
        mnemonic = self.key_gen.generate_mnemonic()
        
        # Derive hotkey
        hotkey = self.key_gen.derive_hotkey(mnemonic, index=0)
        
        assert hotkey is not None
        assert 'private_key' in hotkey
        assert 'public_key' in hotkey
        assert 'address' in hotkey

    def test_derive_multiple_hotkeys(self):
        """Test deriving multiple hotkeys from same mnemonic."""
        mnemonic = self.key_gen.generate_mnemonic()
        
        hotkey0 = self.key_gen.derive_hotkey(mnemonic, index=0)
        hotkey1 = self.key_gen.derive_hotkey(mnemonic, index=1)
        hotkey2 = self.key_gen.derive_hotkey(mnemonic, index=2)
        
        # All should be different
        assert hotkey0['address'] != hotkey1['address']
        assert hotkey1['address'] != hotkey2['address']
        assert hotkey0['address'] != hotkey2['address']

    def test_deterministic_derivation(self):
        """Test that derivation is deterministic."""
        mnemonic = self.key_gen.generate_mnemonic()
        
        # Derive same hotkey twice
        hotkey1 = self.key_gen.derive_hotkey(mnemonic, index=5)
        hotkey2 = self.key_gen.derive_hotkey(mnemonic, index=5)
        
        # Should be identical
        assert hotkey1['address'] == hotkey2['address']
        assert hotkey1['private_key'] == hotkey2['private_key']

    def test_validate_mnemonic(self):
        """Test mnemonic validation."""
        # Valid mnemonic
        valid_mnemonic = self.key_gen.generate_mnemonic()
        assert self.key_gen.validate_mnemonic(valid_mnemonic)
        
        # Invalid mnemonic
        invalid_mnemonic = "invalid word list test"
        assert not self.key_gen.validate_mnemonic(invalid_mnemonic)


class TestKeyEncryption:
    """Test key encryption and decryption."""

    def setup_method(self):
        """Setup test fixtures."""
        self.temp_dir = tempfile.mkdtemp()

    def teardown_method(self):
        """Cleanup temporary directories."""
        if os.path.exists(self.temp_dir):
            shutil.rmtree(self.temp_dir)

    def test_encrypt_decrypt_data(self):
        """Test encrypting and decrypting data."""
        password = "TestPassword123!"
        data = b"sensitive data to encrypt"
        
        # Encrypt
        encrypted = encrypt_data(data, password)
        assert encrypted != data
        assert len(encrypted) > len(data)
        
        # Decrypt
        decrypted = decrypt_data(encrypted, password)
        assert decrypted == data

    def test_wrong_password_fails(self):
        """Test that wrong password fails to decrypt."""
        password = "TestPassword123!"
        wrong_password = "WrongPassword456!"
        data = b"sensitive data"
        
        encrypted = encrypt_data(data, password)
        
        # Try to decrypt with wrong password
        with pytest.raises(Exception):
            decrypt_data(encrypted, wrong_password)

    def test_encrypt_mnemonic(self):
        """Test encrypting mnemonic phrase."""
        mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        password = "TestPassword123!"
        
        encrypted = encrypt_data(mnemonic.encode(), password)
        decrypted = decrypt_data(encrypted, password)
        
        assert decrypted.decode() == mnemonic

    def test_save_and_load_encrypted_file(self):
        """Test saving and loading encrypted file."""
        password = "TestPassword123!"
        data = b"test secret data"
        file_path = Path(self.temp_dir) / "test.enc"
        
        # Encrypt data
        encrypted = encrypt_data(data, password)
        
        # Save to file
        with open(file_path, 'wb') as f:
            f.write(encrypted)
        
        assert file_path.exists()
        
        # Load from file
        with open(file_path, 'rb') as f:
            loaded_encrypted = f.read()
        
        # Decrypt
        decrypted = decrypt_data(loaded_encrypted, password)
        assert decrypted == data


class TestKeyManagementIntegration:
    """Integration tests for complete key management workflow."""

    def setup_method(self):
        """Setup test fixtures."""
        self.key_gen = KeyGenerator()
        self.temp_dir = tempfile.mkdtemp()

    def teardown_method(self):
        """Cleanup temporary directories."""
        if os.path.exists(self.temp_dir):
            shutil.rmtree(self.temp_dir)

    def test_complete_coldkey_workflow(self):
        """Test complete coldkey creation and storage workflow."""
        password = "TestPassword123!"
        
        # 1. Generate mnemonic
        mnemonic = self.key_gen.generate_mnemonic()
        assert len(mnemonic.split()) in [12, 24]
        
        # 2. Encrypt mnemonic
        encrypted_mnemonic = encrypt_data(mnemonic.encode(), password)
        
        # 3. Save to file
        coldkey_file = Path(self.temp_dir) / "coldkey.enc"
        with open(coldkey_file, 'wb') as f:
            f.write(encrypted_mnemonic)
        
        # 4. Load from file
        with open(coldkey_file, 'rb') as f:
            loaded_encrypted = f.read()
        
        loaded_mnemonic = decrypt_data(loaded_encrypted, password).decode()
        
        # 5. Verify
        assert loaded_mnemonic == mnemonic

    def test_complete_hotkey_workflow(self):
        """Test complete hotkey derivation and storage workflow."""
        password = "TestPassword123!"
        
        # 1. Generate coldkey mnemonic
        coldkey_mnemonic = self.key_gen.generate_mnemonic()
        
        # 2. Derive hotkey
        hotkey = self.key_gen.derive_hotkey(coldkey_mnemonic, index=0)
        
        # 3. Handle private key as bytes for security
        privkey_bytes = hotkey['private_key'].encode()
        
        # 4. Encrypt hotkey private key
        encrypted_privkey = encrypt_data(privkey_bytes, password)
        
        # 5. Save to file
        hotkey_file = Path(self.temp_dir) / "hotkey.enc"
        with open(hotkey_file, 'wb') as f:
            f.write(encrypted_privkey)
        
        # 6. Load from file
        with open(hotkey_file, 'rb') as f:
            loaded_encrypted = f.read()
        
        loaded_privkey_bytes = decrypt_data(loaded_encrypted, password)
        loaded_privkey = loaded_privkey_bytes.decode()
        
        # 7. Verify
        assert loaded_privkey == hotkey['private_key']

    def test_multiple_hotkeys_from_coldkey(self):
        """Test creating multiple hotkeys from single coldkey."""
        coldkey_mnemonic = self.key_gen.generate_mnemonic()
        
        # Create 5 hotkeys
        hotkeys = []
        for i in range(5):
            hotkey = self.key_gen.derive_hotkey(coldkey_mnemonic, index=i)
            hotkeys.append(hotkey)
        
        # Verify all are unique
        addresses = [hk['address'] for hk in hotkeys]
        assert len(addresses) == len(set(addresses))
        
        # Verify reproducibility
        hotkey_0_again = self.key_gen.derive_hotkey(coldkey_mnemonic, index=0)
        assert hotkey_0_again['address'] == hotkeys[0]['address']
