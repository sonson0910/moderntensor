# ModernTensor Tokenomics ($MDT)

## 1. Core Philosophy: Adaptive & Sustainable Emission
Khác với Bittensor (in tiền cố định bất kể nhu cầu), ModernTensor áp dụng cơ chế **"Phát thải thích ứng" (Adaptive Emission)**. Mục tiêu là giảm tối đa lạm phát khi mạng lưới chưa tạo ra giá trị thực, và chỉ tăng cung khi nhu cầu sử dụng mạng lưới tăng.

## 2. Key Metrics
*   **Token Name:** ModernTensor ($MDT)
*   **Max Supply:** 21,000,000 MDT
*   **Halving:** Dynamic (Dựa trên tổng cung lưu thông, không cố định theo thời gian).
*   **Emission Type:** Deflationary (Giảm phát).

## 3. Cơ chế Giảm Phác Thải (Emission Reduction Mechanisms)

### A. Utility-Based Minting (In tiền dựa trên Tiện ích)
Thay vì in cố định `1 MDT` mỗi block, lượng in ra (`MintAmount`) phụ thuộc vào **Điểm Tiện ích Mạng lưới (Network Utility Score - $U$)**:

$$ MintAmount = BaseReward \times U $$

Trong đó $U$ (từ 0.0 đến 1.0) được tính dựa trên:
1.  **Task Volume:** Số lượng task AI được giải quyết trong chu kỳ.
2.  **Task Difficulty:** Độ khó trung bình của các task (zkML proof complexity).
3.  **Validator Participation:** Tỷ lệ Validator tham gia đồng thuận.

*   **Hệ quả:** Nếu mạng lưới "vắng khách" (ít task), $U$ thấp $\rightarrow$ Lượng token in ra cực thấp. Tránh lạm phát vô nghĩa.

### B. The Recycling Pool (Bể Tái Chế)
Ưu tiên sử dụng token đã có sẵn thay vì in mới.
1.  **Nguồn thu:**
    *   Phí đăng ký Miner (Registration Fee).
    *   Phí phạt Validator (Slashing).
    *   Phí người dùng gửi Task (nếu có).
2.  **Cơ chế:**
    *   Khi đến kỳ trả thưởng, Smart Contract kiểm tra **Recycling Pool**.
    *   Nếu Pool có tiền $\rightarrow$ Lấy tiền từ Pool trả cho Miner/Validator.
    *   Nếu Pool thiếu $\rightarrow$ Mới kích hoạt Minting để bù phần thiếu.

### C. Burn Mechanism (Cơ chế Đốt)
*   **Unmet Quota Burn:** Nếu trong một chu kỳ, chất lượng mạng lưới kém (không đạt chuẩn đồng thuận), phần thưởng dự kiến của chu kỳ đó sẽ bị **ĐỐT BỎ** thay vì tích lũy.
*   **Transaction Fee Burn:** 50% phí giao dịch trên mạng lưới (nếu có Layer 2 fees) sẽ bị đốt.

## 4. Phân Phối (Distribution)
Mỗi đợt trả thưởng (sau khi đã tính toán giảm phát) được chia như sau:

*   **40% - Miners:** Dựa trên Proof of Useful Work (zkML).
*   **40% - Validators:** Dựa trên Consensus Weight & Staking.
*   **20% - DAO Treasury:** Dùng cho R&D và vận hành hạ tầng (Oracle/Hydra).

## 5. Technical Implementation Plan
1.  **On-Chain Metrics Oracle:** Một Oracle (hoặc Validator Consensus) sẽ feed chỉ số $U$ (Utility Score) lên chuỗi mỗi chu kỳ.
2.  **Smart Minting Policy:** Script Plutus kiểm tra $U$ và số dư Recycling Pool trước khi cho phép Mint.
3.  **Claim Vault:** Người dùng claim token từ Vault (nơi chứa cả token tái chế và token mới in).
