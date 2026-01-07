# Kế hoạch Dọn dẹp Code - ModernTensor SDK

**Ngày:** 2026-01-07  
**Mục đích:** Xác định và loại bỏ code thừa, tổ chức lại cấu trúc file  
**Ưu tiên:** 🟢 Trung bình - Không ảnh hưởng functionality

---

## 📋 Tóm tắt

Sau khi phân tích codebase, phát hiện:
- ✅ **Cấu trúc tổng thể TỐT** - SDK có tổ chức hợp lý
- ⚠️ **Một số file ở sai vị trí** - Cần di chuyển để nhất quán
- ⚠️ **Module "moderntensor" có thể thừa** - Cần xác nhận
- ✅ **Ít duplicate code** - Codebase khá clean

---

## 🎯 Hành động Cụ thể

### 1. Di chuyển Verification Scripts ✅ KHUYẾN NGHỊ

**Hiện tại:** Root directory
```
moderntensor/
├── verify_axon.py
├── verify_dendrite.py  
├── verify_synapse.py
├── verify_integration.py
└── verify_phase3.py
```

**Đề xuất:** Di chuyển vào `tests/integration/`
```
tests/
└── integration/
    ├── verify_axon.py
    ├── verify_dendrite.py
    ├── verify_synapse.py
    ├── verify_integration.py
    └── verify_phase3.py
```

**Lý do:**
- Đây là integration tests, không phải root-level scripts
- Tổ chức tests tốt hơn
- Dễ chạy với pytest

**Lệnh thực hiện:**
```bash
mkdir -p tests/integration
git mv verify_axon.py tests/integration/
git mv verify_dendrite.py tests/integration/
git mv verify_synapse.py tests/integration/
git mv verify_integration.py tests/integration/
git mv verify_phase3.py tests/integration/
```

**Rủi ro:** ⚠️ Thấp - Cần update CI/CD nếu có references

---

### 2. Di chuyển Demo Script ✅ KHUYẾN NGHỊ

**Hiện tại:** Root directory
```
moderntensor/
└── demo_node_lifecycle.py
```

**Đề xuất:** Di chuyển vào `examples/`
```
examples/
└── demo_node_lifecycle.py
```

**Lý do:**
- Đây là demo/example script
- Nhất quán với các examples khác
- Root directory sạch hơn

**Lệnh thực hiện:**
```bash
git mv demo_node_lifecycle.py examples/
```

**Rủi ro:** ⚠️ Rất thấp - Standalone script

---

### 3. Xử lý Module "moderntensor" ⚠️ CẦN XÁC NHẬN

**Hiện tại:** Directory `/moderntensor/`
```
moderntensor/
└── kickoff/  # (có thể rỗng)
```

**Kích thước:** 20KB (nhỏ)

**Đề xuất 3 options:**

#### Option A: ❌ XÓA (nếu không sử dụng)
```bash
# Kiểm tra trước
git log -- moderntensor/
grep -r "from moderntensor" . 
grep -r "import moderntensor" .

# Nếu không có references
git rm -r moderntensor/
```

#### Option B: ✅ DOCUMENT (nếu cần giữ)
- Thêm README.md trong moderntensor/ giải thích mục đích
- Thêm comment trong các files

#### Option C: 🔄 INTEGRATE (nếu có logic quan trọng)
- Di chuyển code vào SDK
- Xóa directory

**Hành động cần thiết:**
1. ✅ Kiểm tra git history của thư mục này
2. ✅ Tìm kiếm imports trong codebase
3. ✅ Xác định mục đích ban đầu
4. ✅ Quyết định: Xóa / Document / Integrate

**Rủi ro:** ⚠️ Trung bình - Cần xác nhận trước khi xóa

---

### 4. Review Duplicate Examples vs Verify Scripts ⚠️ CẦN KIỂM TRA

**Files cần review:**

| Example | Verify Script | Có duplicate? |
|---------|---------------|---------------|
| `examples/axon_example.py` | `verify_axon.py` | ❓ Cần check |
| `examples/dendrite_example.py` | `verify_dendrite.py` | ❓ Cần check |
| `examples/synapse_example.py` | `verify_synapse.py` | ❓ Cần check |

**Hành động:**
1. ✅ So sánh nội dung từng cặp files
2. ✅ Xác định:
   - Nếu **duplicate** → Merge hoặc xóa 1 file
   - Nếu **khác mục đích** → Document rõ ràng khác biệt

**Script để check:**
```bash
# So sánh từng cặp
diff examples/axon_example.py verify_axon.py
diff examples/dendrite_example.py verify_dendrite.py
diff examples/synapse_example.py verify_synapse.py
```

**Quyết định:**
- **Examples:** Hướng dẫn sử dụng cho developers → Giữ
- **Verify:** Tests/validation → Di chuyển vào tests/

**Rủi ro:** ⚠️ Thấp - Chỉ organization

---

### 5. Review `sdk/runner.py` ⚠️ CẦN XÁC NHẬN

**File:** `sdk/runner.py` (252 dòng)

**Câu hỏi:**
- ❓ Mục đích của file này là gì?
- ❓ Có được sử dụng không?
- ❓ Có trùng với CLI entry points không?
- ❓ Có được import từ đâu không?

**Hành động kiểm tra:**
```bash
# Tìm usages
grep -r "from sdk.runner" .
grep -r "import runner" .
grep -r "runner.py" .

# Check git history
git log -- sdk/runner.py
```

**Quyết định dựa trên kết quả:**
- Nếu **được sử dụng** → Document mục đích
- Nếu **trùng với CLI** → Merge vào CLI
- Nếu **không sử dụng** → Xóa

**Rủi ro:** ⚠️ Trung bình - Cần xác nhận trước

---

### 6. Tối ưu Network Module Structure ⚠️ OPTIONAL

**Hiện tại:** `sdk/network/` (29 files, 1,629 dòng)

**Đề xuất:** Review và refactor nếu cần

**Hành động:**
1. ✅ List tất cả files với kích thước
2. ✅ Xác định files có logic duplicate
3. ✅ Refactor nếu có nhiều small files làm cùng việc

**Script:**
```bash
find sdk/network -name "*.py" -exec wc -l {} + | sort -n
```

**Quyết định:** Dựa trên review chi tiết

**Rủi ro:** ⚠️ Cao - Có thể break functionality

**Khuyến nghị:** ⏸️ Postpone - Không ưu tiên cao

---

### 7. Tối ưu AI/ML Module Structure ⚠️ OPTIONAL

**Hiện tại:** `sdk/ai_ml/` (22 files, 3,669 dòng, 8 subdirs)

**Đề xuất:** Review structure

**Hành động:**
1. ✅ Review subdirectory organization
2. ✅ Đảm bảo không có duplicate
3. ✅ Refactor nếu structure phức tạp không cần thiết

**Quyết định:** Dựa trên review chi tiết

**Rủi ro:** ⚠️ Cao - Có thể break functionality

**Khuyến nghị:** ⏸️ Postpone - Module này đang hoạt động tốt (70% complete)

---

## 📊 Tổng kết Hành động

### 🟢 Ưu tiên Cao - Làm ngay (Tuần này)

✅ **An toàn, không break code:**

1. **Di chuyển verify scripts vào tests/**
   - Impact: Low
   - Risk: Low
   - Time: 10 phút
   - Status: ✅ READY TO EXECUTE

2. **Di chuyển demo script vào examples/**
   - Impact: Low
   - Risk: Very Low
   - Time: 5 phút
   - Status: ✅ READY TO EXECUTE

### 🟡 Ưu tiên Trung bình - Cần kiểm tra (Tuần 2)

⚠️ **Cần xác nhận trước:**

3. **Xác định và xử lý module moderntensor/**
   - Impact: Low-Medium
   - Risk: Medium
   - Time: 30 phút investigate + action
   - Status: ⏸️ NEEDS INVESTIGATION

4. **Review duplicate examples vs verify**
   - Impact: Low
   - Risk: Low
   - Time: 1 giờ
   - Status: ⏸️ NEEDS REVIEW

5. **Xác định mục đích sdk/runner.py**
   - Impact: Low-Medium
   - Risk: Medium
   - Time: 30 phút investigate + action
   - Status: ⏸️ NEEDS INVESTIGATION

### 🔵 Ưu tiên Thấp - Optional (Sau này)

⏸️ **Không cấp thiết:**

6. **Refactor sdk/network/ nếu cần**
   - Impact: Medium
   - Risk: High
   - Time: 2-4 giờ
   - Status: ⏸️ POSTPONED

7. **Refactor sdk/ai_ml/ nếu cần**
   - Impact: Medium
   - Risk: High
   - Time: 2-4 giờ
   - Status: ⏸️ POSTPONED

---

## 🚀 Implementation Plan

### Week 1: Quick Wins

**Day 1-2:**
```bash
# 1. Di chuyển verify scripts
mkdir -p tests/integration
git mv verify_*.py tests/integration/

# 2. Di chuyển demo script
git mv demo_node_lifecycle.py examples/

# 3. Commit
git commit -m "chore: Reorganize verification and demo scripts

- Move verify_*.py to tests/integration/
- Move demo_node_lifecycle.py to examples/
- Improve project organization"
```

**Day 3-4:**
```bash
# 4. Investigate moderntensor/ module
git log -- moderntensor/
grep -r "from moderntensor" .
grep -r "import moderntensor" .

# 5. Make decision and document
# Create DECISION.md with findings
```

**Day 5:**
```bash
# 6. Investigate runner.py
git log -- sdk/runner.py
grep -r "from sdk.runner" .
grep -r "runner" pyproject.toml setup.py

# 7. Make decision and document
```

### Week 2: Reviews

**Day 1-2:**
- Compare examples vs verify scripts
- Document differences or merge if duplicate

**Day 3-5:**
- Update documentation với new structure
- Update CI/CD if needed
- Test everything still works

---

## ✅ Checklist Thực hiện

### Immediate (Tuần này):

- [ ] Di chuyển `verify_*.py` → `tests/integration/`
- [ ] Di chuyển `demo_node_lifecycle.py` → `examples/`
- [ ] Commit changes
- [ ] Verify tests still pass

### Investigation (Tuần 2):

- [ ] Check git history: `moderntensor/`
- [ ] Search imports: `moderntensor`
- [ ] Decision: Delete / Document / Integrate
- [ ] Check git history: `sdk/runner.py`
- [ ] Search usages: `runner.py`
- [ ] Decision: Keep / Merge / Delete

### Review (Tuần 2-3):

- [ ] Compare `examples/*_example.py` vs `verify_*.py`
- [ ] Identify duplicates
- [ ] Merge or document differences

### Documentation (Tuần 3):

- [ ] Update README với new structure
- [ ] Document decisions made
- [ ] Update CI/CD configs if needed

---

## 📝 Expected Results

### Before:
```
moderntensor/
├── verify_axon.py           # ❌ Wrong location
├── verify_dendrite.py       # ❌ Wrong location
├── verify_synapse.py        # ❌ Wrong location
├── verify_integration.py    # ❌ Wrong location
├── verify_phase3.py         # ❌ Wrong location
├── demo_node_lifecycle.py   # ❌ Wrong location
├── moderntensor/            # ❓ Purpose unclear
│   └── kickoff/
├── sdk/
│   ├── runner.py            # ❓ Purpose unclear
│   └── ...
├── examples/
│   ├── axon_example.py      # ❓ Maybe duplicate?
│   └── ...
└── tests/
    └── ...
```

### After:
```
moderntensor/
├── sdk/
│   ├── [runner.py or documented] # ✅ Clear purpose
│   └── ...
├── examples/
│   ├── demo_node_lifecycle.py   # ✅ Organized
│   ├── axon_example.py           # ✅ Documented
│   └── ...
└── tests/
    ├── integration/
    │   ├── verify_axon.py        # ✅ Organized
    │   ├── verify_dendrite.py    # ✅ Organized
    │   └── ...
    └── ...
```

---

## 🎯 Success Metrics

✅ **Organization Improved:**
- Verification scripts in proper location
- Demo scripts in examples/
- Clear purpose for all modules

✅ **No Functionality Broken:**
- All tests still pass
- CI/CD still works
- No import errors

✅ **Documentation Updated:**
- README reflects new structure
- Purpose of all modules documented
- Decisions recorded

---

## ⚠️ Risks & Mitigation

### Risk 1: Breaking imports
**Mitigation:** Search all imports before moving
```bash
grep -r "import verify_" .
grep -r "from . import verify_" .
```

### Risk 2: CI/CD failures
**Mitigation:** Check CI configs before moving
```bash
cat .github/workflows/*.yml | grep verify
```

### Risk 3: Deleting important code
**Mitigation:** 
- Always check git history first
- Search for all usages
- Create backup branch before deleting

---

## 📞 Questions to Answer

Before executing, answer:

1. ❓ **moderntensor/ module:**
   - What is its purpose?
   - Is it imported anywhere?
   - Can we safely delete it?

2. ❓ **sdk/runner.py:**
   - What does it do?
   - Is it used?
   - Should it be part of CLI?

3. ❓ **Examples vs Verify:**
   - Are they duplicates?
   - What's the difference?
   - Should we merge them?

---

## 🎬 Conclusion

**Hiện trạng:** 
- Codebase khá clean
- Chỉ có vài files ở sai vị trí
- Không nhiều duplicate code

**Kế hoạch:**
- 🟢 Quick wins: Di chuyển files (1 tuần)
- 🟡 Investigations: Xác nhận purpose (1 tuần)
- 🔵 Reviews: Optional refactoring (sau này)

**Tác động:**
- ✅ Minimal risk
- ✅ Better organization
- ✅ Clearer structure

---

**Document này:** Kế hoạch chi tiết để dọn dẹp code  
**Status:** ✅ SẴN SÀNG THỰC HIỆN  
**Next:** Execute Week 1 actions → Report progress
