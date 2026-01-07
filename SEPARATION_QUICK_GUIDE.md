# LuxTensor Separation - Quick Reference Guide

**Date:** January 7, 2026  
**Purpose:** Quick guide for separating LuxTensor into its own repository

---

## 🎯 Quick Answer

**Question:** "Should I extract just the luxtensor folder, or what else?"

**Answer:** ✅ **Extract the entire `luxtensor/` directory** + related Rust documentation files

---

## 📦 What to Extract

### 1. Main Directory (Must Extract)

```
✅ luxtensor/                        # Entire folder
   ├── crates/                       # All 10 Rust crates
   ├── examples/                     # Rust examples
   ├── .github/workflows/            # Rust CI/CD
   ├── .cargo/                       # Cargo config
   ├── Cargo.toml                    # Workspace manifest
   ├── rust-toolchain.toml          # Rust version
   ├── .rustfmt.toml                # Code formatting
   ├── .gitignore                    # Rust-specific
   ├── Dockerfile.rust              # Docker image
   ├── README.md                     # LuxTensor docs
   ├── config.*.toml                # Configuration
   └── genesis.testnet.json         # Genesis data
```

### 2. Documentation Files (Should Extract)

```
✅ RUST_MIGRATION_ROADMAP.md
✅ RUST_MIGRATION_SUMMARY_VI.md
✅ LUXTENSOR_SETUP.md
✅ LUXTENSOR_PROGRESS.md
✅ LUXTENSOR_COMPLETION_SUMMARY.md
✅ LUXTENSOR_FINAL_COMPLETION.md
✅ LUXTENSOR_USAGE_GUIDE.md
```

### 3. License & Config (Copy)

```
📄 LICENSE                           # Copy to both repos
📄 .gitignore                        # Customize for each repo
```

---

## ❌ What NOT to Extract

**Keep in ModernTensor:**

```
❌ sdk/                              # Python SDK
❌ tests/                            # Python tests
❌ examples/                         # Python examples (not Rust)
❌ pyproject.toml                    # Python config
❌ requirements.txt                  # Python dependencies
❌ README.md                         # Main README (update links)
❌ docs/                             # Main documentation
```

---

## 🚀 Step-by-Step Instructions

### Step 1: Create New Repository

1. Go to GitHub: https://github.com/new
2. Repository name: `luxtensor`
3. Description: "High-performance Layer 1 blockchain for ModernTensor (Rust)"
4. Public repository
5. **Do NOT** initialize with README (will push existing code)
6. License: MIT
7. Click "Create repository"

### Step 2: Extract LuxTensor

```bash
# Clone current repository
git clone https://github.com/sonson0910/moderntensor
cd moderntensor

# Create new directory for LuxTensor
mkdir ../luxtensor-new
cd ../luxtensor-new

# Copy luxtensor content
cp -r ../moderntensor/luxtensor/* .
cp -r ../moderntensor/luxtensor/.github .
cp -r ../moderntensor/luxtensor/.cargo .
cp ../moderntensor/luxtensor/.rustfmt.toml .
cp ../moderntensor/luxtensor/.gitignore .

# Copy documentation
cp ../moderntensor/RUST_MIGRATION*.md .
cp ../moderntensor/LUXTENSOR*.md .
cp ../moderntensor/LICENSE .

# Initialize git
git init
git add .
git commit -m "Initial commit: Extract LuxTensor from ModernTensor"

# Push to new repository
git remote add origin https://github.com/sonson0910/luxtensor
git branch -M main
git push -u origin main
```

### Step 3: Update ModernTensor

```bash
# Go back to moderntensor
cd ../moderntensor

# Create new branch
git checkout -b refactor/separate-luxtensor

# Remove luxtensor directory
rm -rf luxtensor/

# Remove Rust documentation
rm -f LUXTENSOR*.md
rm -f RUST_MIGRATION*.md

# Update README.md (add link to LuxTensor repo)
# ... edit README.md ...

# Commit changes
git add .
git commit -m "refactor: Move LuxTensor to separate repository

- Removed luxtensor/ directory
- Updated README with links to LuxTensor repo
- ModernTensor now focuses on Python SDK

New LuxTensor repo: https://github.com/sonson0910/luxtensor"

git push origin refactor/separate-luxtensor
```

### Step 4: Update README Files

**LuxTensor README.md** - Add ecosystem section:

```markdown
## Ecosystem

- **LuxTensor (this repo)** - High-performance Layer 1 blockchain (Rust)
- **[ModernTensor](https://github.com/sonson0910/moderntensor)** - Python SDK

## Using with ModernTensor

```python
pip install moderntensor

from sdk import LuxTensorClient
client = LuxTensorClient("http://localhost:9933")
```
```

**ModernTensor README.md** - Add architecture section:

```markdown
## Architecture

- **ModernTensor (this repo)** - Python SDK, CLI tools
- **[LuxTensor](https://github.com/sonson0910/luxtensor)** - Rust blockchain

## Running LuxTensor Node

```bash
git clone https://github.com/sonson0910/luxtensor
cd luxtensor
cargo build --release
./target/release/luxtensor-node
```
```

---

## ✅ Verification Checklist

### After Separation

- [ ] LuxTensor repository created and pushed
- [ ] All Rust code in LuxTensor repo
- [ ] All Python code remains in ModernTensor repo
- [ ] README files updated with cross-references
- [ ] CI/CD works on both repositories
- [ ] No broken links in documentation
- [ ] LICENSE file in both repos
- [ ] GitHub repository descriptions updated

### Test Both Repos

**Test LuxTensor:**
```bash
git clone https://github.com/sonson0910/luxtensor
cd luxtensor
cargo test --workspace
cargo build --release
```

**Test ModernTensor:**
```bash
git clone https://github.com/sonson0910/moderntensor
cd moderntensor
pip install -e .
pytest tests/
```

---

## 📊 Repository Comparison

| Aspect | ModernTensor | LuxTensor |
|--------|-------------|-----------|
| **Language** | Python | Rust |
| **Purpose** | SDK for developers | Production blockchain |
| **Files** | 159 Python files | 77 Rust files |
| **LOC** | ~22,000 Python | ~15,000 Rust (target) |
| **Target Users** | App developers | Node operators |
| **Performance** | 50-100 TPS | 1,000-5,000 TPS |
| **CI/CD** | Python tests | Rust tests |
| **Release Cycle** | Independent | Independent |

---

## 🎯 Benefits of Separation

1. ✅ **Clear Focus**
   - ModernTensor: Developer experience
   - LuxTensor: Infrastructure & performance

2. ✅ **Independent Development**
   - Python team works on SDK
   - Rust team works on blockchain

3. ✅ **Better CI/CD**
   - Faster builds (no mixed language tests)
   - Clear deployment pipelines

4. ✅ **Improved Discoverability**
   - 2 GitHub repositories = better SEO
   - Clear project boundaries

5. ✅ **Easier Contribution**
   - Contributors can focus on one language
   - Smaller, more focused repositories

---

## 🚨 Common Pitfalls to Avoid

❌ **Don't:** Keep both Python and Rust in one repo  
✅ **Do:** Separate by language and purpose

❌ **Don't:** Use git submodules (too complex)  
✅ **Do:** Simple repository separation with cross-links

❌ **Don't:** Forget to update documentation  
✅ **Do:** Update all README files and links

❌ **Don't:** Break existing workflows  
✅ **Do:** Test both repos after separation

---

## 📞 Questions?

**For ModernTensor (Python SDK):**
- Repository: https://github.com/sonson0910/moderntensor
- Issues: https://github.com/sonson0910/moderntensor/issues

**For LuxTensor (Rust Blockchain):**
- Repository: https://github.com/sonson0910/luxtensor
- Issues: https://github.com/sonson0910/luxtensor/issues

---

## Summary

**Answer to your question:**

> "Giờ nên lấy những gì ra ngoài, hay là lấy mình thư mục luxtensor ra ngoài thôi?"

✅ **Extract the entire `luxtensor/` directory + Rust documentation files**

This is the simplest and cleanest approach:
- LuxTensor is already independent (no Python dependencies)
- Clear separation of concerns (Python SDK vs Rust blockchain)
- Easy to manage and develop independently
- Better visibility and discoverability

**Just extract `luxtensor/` folder - that's it!** 🚀
