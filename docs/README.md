# Moderntensor: Tài liệu Chi tiết về Cơ chế Hoạt động và Công thức Cốt lõi

**Nhóm Phát triển Moderntensor**
*(Cập nhật lần cuối: [Ngày cập nhật thực tế])*

## 1. Tóm tắt (Abstract)

Tài liệu này cung cấp một cái nhìn tổng quan và chi tiết về các công thức toán học và cơ chế hoạt động cốt lõi của Moderntensor - một nền tảng Trí tuệ Nhân tạo (AI) phi tập trung được xây dựng trên blockchain Cardano [source: 76, 79]. Các cơ chế được trình bày bao gồm phân phối khuyến khích (incentive), đánh giá hiệu suất (performance), xử lý vi phạm (penalty), tính toán trọng số (weighting), phân bổ tài nguyên và các cơ chế bổ sung như quản trị DAO [source: 77]. Mỗi công thức được mô tả rõ ràng bằng ký hiệu toán học, kèm theo giải thích chi tiết về ý nghĩa, mục đích, các tham số liên quan và ví dụ minh họa [source: 78].

## 2. Giới thiệu (Introduction)

Moderntensor là một nền tảng AI phi tập trung, tận dụng sức mạnh của công nghệ blockchain Cardano để tạo ra một môi trường minh bạch, công bằng và hiệu quả cho việc huấn luyện, xác thực và khen thưởng các mô hình AI [source: 79]. Tài liệu này tập trung phân tích các công thức nền tảng điều khiển các cơ chế kinh tế, đánh giá và quản trị của hệ thống [source: 80]. Mục tiêu là cung cấp sự hiểu biết sâu sắc về logic hoạt động và các cân nhắc thiết kế đằng sau Moderntensor.

## 3. Các Cơ chế Cốt lõi

### 3.1. Phân phối Khuyến khích (Incentive Distribution)

Cơ chế khuyến khích được thiết kế để đảm bảo phần thưởng được phân phối công bằng đến các thành phần tham gia (Miners và Validators) dựa trên sự đóng góp, hiệu suất và độ tin cậy của họ.

#### 3.1.1. Khuyến khích cho Miners

Phần thưởng cho Miner ($Incentive_{miner}$) được tính toán dựa trên điểm tin cậy, trọng số và điểm hiệu suất từ các Validators.

**Công thức:** [source: 81]
$Incentive_{miner}(x) = f(trust_{score}(x)) \times \frac{\sum_{j=0}^{m}(W_{x} \times P_{xj})}{\sum_{i=0}^{n}\sum_{j=0}^{m}(W_{i} \times P_{ij})}$

**Ý nghĩa:** Công thức này đảm bảo rằng các Miner có hiệu suất cao và độ tin cậy tốt (phản ánh qua `trust_score` và `P_xj`) sẽ nhận được phần thưởng xứng đáng, có tính đến trọng số ($W_x$) và được chuẩn hóa theo tổng giá trị đóng góp của toàn hệ thống.

**Tham số:** [source: 82, 83]
* $trust_{score}(x)$: Điểm tin cậy lịch sử của Miner $x$.
* $f(trust_{score}(x))$: Một hàm của điểm tin cậy (thay vì nhân trực tiếp) để điều chỉnh độ nhạy của phần thưởng. *Xem Cân nhắc Thiết kế.*
* $W_x$: Trọng số của Miner $x$ (có thể dựa trên stake, hiệu suất lịch sử, hoặc kết hợp). *Xem Công thức 3.6.1.*
* $P_{xj}$: Điểm hiệu suất của Miner $x$ được đánh giá bởi Validator $j$. *Xem Công thức 3.5.*
* Mẫu số: Tổng giá trị đóng góp có trọng số của tất cả Miners trong hệ thống, dùng để chuẩn hóa phần thưởng.

**Ví dụ:** (Giả sử $f(trust) = trust$ như công thức gốc [source: 84, 85])
Miner $x$ có $W_x=2$, $trust_{score}(x)=0.9$. Được đánh giá bởi 3 validators: $P_{x1}=0.8, P_{x2}=0.9, P_{x3}=0.7$. Tổng giá trị hệ thống $\sum \sum (W_i \times P_{ij}) = 50$.
$Incentive_{miner}(x) = 0.9 \times \frac{2 \times (0.8 + 0.9 + 0.7)}{50} = 0.9 \times \frac{2 \times 2.4}{50} = 0.0864$

**Cân nhắc Thiết kế (Miner Incentive):**
* **Ảnh hưởng của Trust Score:** Công thức gốc nhân trực tiếp với `trust_score`. Cân nhắc sử dụng hàm $f(trust\_score)$ tăng nhưng "bão hòa" gần 1.0 (ví dụ: sigmoid hoặc hàm tuyến tính có giới hạn) để giảm biến động phần thưởng khi trust score thay đổi nhỏ ở mức cao.
* **Tính toán Trọng số $W_x$:** Việc xác định trọng số Miner cần cân bằng giữa hiệu suất lịch sử và khả năng chống thao túng. *Xem mục 3.6.1.*

#### 3.1.2. Khuyến khích cho Validators

Validators ($Incentive_{validator}$) được thưởng dựa trên điểm tin cậy, trọng số và hiệu suất đánh giá của chính họ.

**Công thức:** [source: 86]
$Incentive_{validator}(v) = f(trust_{score}(v)) \times \frac{W_{v} \times E_{v}}{\sum_{u \in V}(W_{u} \times E_{u})}$

**Ý nghĩa:** Khuyến khích Validators duy trì hiệu suất cao ($E_v$) và độ tin cậy ($trust_{score}$), với phần thưởng tỷ lệ thuận với đóng góp của họ (thể hiện qua $W_v$) so với toàn bộ nhóm Validators.

**Tham số:** [source: 87]
* $trust_{score}(v)$: Điểm tin cậy lịch sử của Validator $v$.
* $f(trust_{score}(v))$: Hàm điều chỉnh ảnh hưởng của trust score (tương tự như Miner).
* $W_v$: Trọng số của Validator $v$. *Xem Công thức 3.6.2.*
* $E_v$: Điểm hiệu suất của Validator $v$. *Xem Công thức 3.3.*
* Mẫu số: Tổng giá trị đóng góp có trọng số của tất cả Validators, dùng để chuẩn hóa.

**Ví dụ:** (Giả sử $f(trust) = trust$ như công thức gốc)
Validator $v$ có $W_v=3$, $E_v=0.95$, $trust_{score}(v)=0.85$. Tổng giá trị Validators $\sum (W_u \times E_u) = 60$.
$Incentive_{validator}(v) = 0.85 \times \frac{3 \times 0.95}{60} = 0.85 \times \frac{2.85}{60} \approx 0.0404$

**Cân nhắc Thiết kế (Validator Incentive):**
* **Tính toán Trọng số $W_v$:** Sự cân bằng giữa stake và hiệu suất trong $W_v$ là rất quan trọng. *Xem mục 3.6.2.*

### 3.2. Đánh giá Hiệu suất (Performance Evaluation)

Hệ thống sử dụng nhiều chỉ số để đánh giá hiệu suất của Miners và Validators một cách toàn diện.

#### 3.2.1. Tỷ lệ Hoàn thành Nhiệm vụ ($Q_{task}$)

Đo lường tỷ lệ hoàn thành nhiệm vụ thành công, ưu tiên các nhiệm vụ gần đây.

**Công thức:** [source: 88]
$Q_{task} = \frac{\sum_{t}(task_{success,t} \times e^{-\delta(T-t)})}{\sum_{t}(task_{total,t} \times e^{-\delta(T-t)})}$

**Ý nghĩa:** Đánh giá hiệu quả hoàn thành công việc, sử dụng hàm suy giảm mũ $e^{-\delta(T-t)}$ để giảm trọng số của các nhiệm vụ cũ.

**Tham số:** [source: 88, 89]
* $task_{success,t}$: Số lượng nhiệm vụ hoàn thành thành công tại thời điểm $t$.
* $task_{total,t}$: Tổng số nhiệm vụ được giao tại thời điểm $t$.
* $T$: Thời điểm hiện tại.
* $\delta$: Hằng số suy giảm (ví dụ: 0.5), kiểm soát tốc độ "quên" nhiệm vụ cũ. *Tham số này có thể do DAO quản trị.*

**Ví dụ:** [source: 89, 90]
Một miner có dữ liệu: $t=1$: 8/10, $t=2$: 9/10, $t=3$: 10/10. Với $T=3$, $\delta=0.5$.
Tử số $\approx 8 \times e^{-1} + 9 \times e^{-0.5} + 10 \times e^{0} \approx 18.407$
Mẫu số $\approx 10 \times e^{-1} + 10 \times e^{-0.5} + 10 \times e^{0} \approx 19.75$
$Q_{task} \approx 18.407 / 19.75 \approx 0.932$ (93.2%)

#### 3.2.2. Tỷ lệ Phê duyệt Miner ($D_{miner}$)

Đo lường tần suất công việc của Miner được Validators phê duyệt.

**Công thức:** [source: 91]
$D_{miner}(i) = \frac{\sum(\text{Số lần Miner } i \text{ được phê duyệt bởi Validators})}{\text{Tổng số lần Miner } i \text{ được kiểm tra}}$

**Ý nghĩa:** Một chỉ số đơn giản về sự chấp nhận kết quả của Miner bởi mạng lưới xác thực.

**Ví dụ:** [source: 91]
Miner $i$ được kiểm tra 10 lần, phê duyệt 8 lần: $D_{miner}(i) = 8 / 10 = 0.8$ (80%).

#### 3.2.3. Điểm Hiệu suất Validator ($E_{validator}$)

Đánh giá hiệu suất tổng hợp của Validator dựa trên nhiều yếu tố.

**Công thức gốc:** [source: 92]
$E_{validator} = \theta_1 Q_{task\_validator} + \theta_2 accuracy_{validator} + \theta_3 \times e^{-k \frac{|Eval - Avg|}{\sigma}}$

**Ý nghĩa:** Kết hợp khả năng hoàn thành nhiệm vụ ($Q_{task}$ của chính validator nếu họ cũng làm task), độ chính xác trong việc đánh giá Miners ($accuracy$) và mức độ đồng thuận với các Validators khác (thành phần $e^{-k...}$ phạt nếu đánh giá quá lệch so với trung bình $Avg$).

**Tham số:** [source: 93]
* $\theta_1, \theta_2, \theta_3$: Các hệ số trọng số ($\sum \theta_i = 1$). *Có thể do DAO quản trị.*
* $Q_{task\_validator}$: Tỷ lệ hoàn thành nhiệm vụ của chính Validator (nếu có).
* $accuracy_{validator}$: Độ chính xác trong đánh giá của Validator. *Cần định nghĩa rõ cách đo lường.*
* $Eval$: Điểm đánh giá của Validator hiện tại.
* $Avg$: Điểm đánh giá trung bình của tất cả Validators cho cùng một đối tượng.
* $\sigma$: Độ lệch chuẩn của các điểm đánh giá.
* $k$: Hệ số kiểm soát mức độ phạt độ lệch. *Có thể do DAO quản trị.*

**Ví dụ:** [source: 94, 95]
Giả sử $Q_{task}=0.9, accuracy=0.85, \frac{|Eval-Avg|}{\sigma}=0.2, k=1, \theta_1=0.4, \theta_2=0.3, \theta_3=0.3$.
$E_{validator} = 0.4 \times 0.9 + 0.3 \times 0.85 + 0.3 \times e^{-0.2} \approx 0.36 + 0.255 + 0.245 = 0.86$

**Cân nhắc Thiết kế (Validator Performance):**
* **Đo lường `accuracy_validator`:** Đây là thách thức. Cần định nghĩa một phương pháp khách quan, ví dụ: dựa trên sự nhất quán lịch sử, sự đồng thuận với validator uy tín, hoặc sử dụng các "nhiệm vụ kiểm tra" (honeypot tasks) có kết quả biết trước.
* **Thành phần Phạt Độ lệch:** Cơ chế `exp(-k * dev)` có thể quá nhạy cảm. Cân nhắc các hàm phạt khác (ví dụ: phạt tuyến tính sau một ngưỡng nhất định) hoặc chỉ so sánh với nhóm validator tin cậy nhất. Tham số `k` và các hệ số `θ` nên được DAO xem xét.

#### 3.2.4. Hiệu suất Miner Cơ bản ($P_{miner}$)

Tương tự $Q_{task}$, đo lường hiệu suất hoàn thành nhiệm vụ của Miner với trọng số suy giảm theo thời gian.

**Công thức:** [source: 96] (Giống hệt công thức $Q_{task}$ [source: 88])
$P_{miner} = \frac{\sum_{t}(task_{success,t} \times e^{-\delta(T-t)})}{\sum_{t}(task_{total,t} \times e^{-\delta(T-t)})}$

**Ý nghĩa, Tham số, Ví dụ:** Tương tự như mục 3.2.1.

#### 3.2.5. Hiệu suất Miner Điều chỉnh ($P_{miner\_adjusted}$)

Điều chỉnh điểm hiệu suất của Miner dựa trên độ tin cậy của các Validators đã đánh giá họ.

**Công thức:** [source: 98]
$P_{miner\_adjusted} = \frac{\sum_{v}(trust_{score_{v}} \times P_{miner,v})}{\sum_{v}trust_{score_{v}}}$

**Ý nghĩa:** Giảm thiểu tác động của các đánh giá từ những Validators có điểm tin cậy thấp, tăng độ tin cậy của điểm hiệu suất cuối cùng.

**Tham số:** [source: 99, 100]
* $trust_{score_v}$: Điểm tin cậy của Validator $v$.
* $P_{miner,v}$: Điểm hiệu suất của Miner được đánh giá bởi Validator $v$.

**Ví dụ:** [source: 100, 101]
Miner M1 được V1 ($trust=0.8$) đánh giá $P_{miner,1}=0.9$, và V2 ($trust=0.5$) đánh giá $P_{miner,2}=0.7$.
$P_{miner\_adjusted} = \frac{(0.8 \times 0.9) + (0.5 \times 0.7)}{0.8 + 0.5} = \frac{0.72 + 0.35}{1.3} \approx 0.823$

### 3.3. Trọng số Người tham gia (Weight Calculation)

Trọng số phản ánh "tầm quan trọng" hoặc "sức ảnh hưởng" của Miner và Validator trong hệ thống, ảnh hưởng đến phần thưởng và các cơ chế khác.

#### 3.3.1. Trọng số Miner ($W_x$)

Tính toán dựa trên hiệu suất lịch sử, ưu tiên các giá trị gần đây.

**Công thức:** [source: 102]
$W_x = \sum_{t} P_{miner,t} \times e^{-\delta(T-t)}$

**Ý nghĩa:** Tổng hợp hiệu suất lịch sử của Miner, khuyến khích duy trì hiệu suất cao và ổn định vì hiệu suất gần đây có tác động lớn hơn.

**Tham số:** [source: 102]
* $P_{miner,t}$: Hiệu suất của Miner tại thời điểm $t$.
* $T$: Thời điểm hiện tại.
* $\delta$: Hằng số suy giảm (ví dụ: 0.5). *Có thể do DAO quản trị.*

**Ví dụ:** [source: 103, 104]
Miner có hiệu suất: $t=1: 0.8, t=2: 0.9, t=3: 1.0$. Với $T=3, \delta=0.5$.
$W_x = 0.8 \times e^{-1} + 0.9 \times e^{-0.5} + 1.0 \times e^{0} \approx 0.294 + 0.546 + 1.0 = 1.84$

#### 3.3.2. Trọng số Validator ($W_{validator}$)

Kết hợp giữa lượng stake, hiệu suất và thời gian tham gia.

**Công thức:** [source: 119]
$W_{validator} = \lambda \times \frac{stake_v}{\sum stake} + (1 - \lambda) \times E_{validator} \times (1 + log(time\_participated))$

**Ý nghĩa:** Cân bằng giữa đóng góp tài chính (stake) và đóng góp về chất lượng (hiệu suất $E_{validator}$), đồng thời ghi nhận sự cam kết lâu dài qua thời gian tham gia ($time\_participated$).

**Tham số:** [source: 119]
* $stake_v$: Lượng stake của Validator $v$.
* $\sum stake$: Tổng lượng stake của tất cả Validators.
* $E_{validator}$: Điểm hiệu suất của Validator $v$. *Xem Công thức 3.3.*
* $time\_participated$: Thời gian Validator $v$ đã tham gia hệ thống (cần định nghĩa đơn vị rõ ràng).
* $\lambda$: Hệ số cân bằng (0 đến 1), quyết định tỷ trọng giữa stake và hiệu suất/thời gian. *Tham số quan trọng, nên do DAO quản trị.*
* $log()$: Hàm logarit (cơ số tự nhiên hoặc 10) để giảm dần lợi ích của thời gian tham gia.

**Ví dụ:** [source: 120]
Giả sử $stake_v=500, \sum stake=2000, E_{validator}=0.9, time=10$ (đơn vị tùy chọn), $\lambda=0.5$.
$W_{validator} = 0.5 \times \frac{500}{2000} + (1 - 0.5) \times 0.9 \times (1 + log_{10}(10))$
$W_{validator} = 0.5 \times 0.25 + 0.5 \times 0.9 \times (1 + 1) = 0.125 + 0.5 \times 0.9 \times 2 = 0.125 + 0.9 = 1.025$
*(Lưu ý: Ví dụ gốc [source: 120] có thể đã dùng log cơ số tự nhiên hoặc có tính toán khác, kết quả là 0.71)*

**Cân nhắc Thiết kế (Validator Weight):**
* **Tham số `lambda`:** Quyết định sự cân bằng stake/performance là tối quan trọng và nên được quản trị cẩn thận bởi cộng đồng (DAO).
* **Bình thường hóa Performance:** Xem xét việc sử dụng $E_{validator}$ tương đối (chia cho trung bình mạng lưới) thay vì giá trị tuyệt đối.
* **Stake theo Bậc:** Có thể xem xét các hệ số nhân khác nhau cho thành phần stake dựa trên các bậc stake, thay vì chỉ dùng tỷ lệ tuyến tính, để giảm lợi thế của stake quá lớn.

### 3.4. Điểm Tin cậy và Cơ chế Công bằng (Trust Score & Fairness)

Hệ thống quản lý điểm tin cậy (uy tín) và đảm bảo cơ hội tham gia công bằng cho các Miners.

#### 3.4.1. Cập nhật Điểm Tin cậy (Trust Score Update)

Điểm tin cậy được cập nhật định kỳ, có tính đến hiệu suất mới và suy giảm do không hoạt động.

**Công thức:** [source: 105]
$TrustScore_{new} = TrustScore_{old} \times e^{-\delta_{trust} \times time\_since\_last\_evaluation} + \alpha \times f_{update}(Score_{new})$

**Ý nghĩa:** Giảm điểm tin cậy theo thời gian nếu không được đánh giá (khuyến khích tham gia tích cực) và cập nhật dựa trên điểm hiệu suất mới nhất ($Score_{new}$) với một tỷ lệ học ($\alpha$).

**Tham số:** [source: 105, 106]
* $TrustScore_{old/new}$: Điểm tin cậy cũ/mới.
* $\delta_{trust}$: Hằng số suy giảm riêng cho trust score (ví dụ: 0.1). *Có thể do DAO quản trị.*
* $time\_since\_last\_evaluation$: Số chu kỳ/thời gian kể từ lần đánh giá cuối.
* $\alpha$: Tỷ lệ học (learning rate, ví dụ: 0.1). *Có thể do DAO quản trị, hoặc linh động.*
* $Score_{new}$: Điểm hiệu suất mới nhận được (ví dụ: $P_{miner\_adjusted}$ hoặc $E_{validator}$). Nếu không được đánh giá trong chu kỳ, $Score_{new}=0$. [source: 107]
* $f_{update}(Score_{new})$: Hàm ánh xạ điểm số mới trước khi cập nhật (ví dụ: hàm tuyến tính hoặc sigmoid). *Xem Cân nhắc Thiết kế.*

**Ví dụ:** [source: 107, 108]
Miner M5 có $TrustScore_{old}=0.5$, không được chọn trong 2 chu kỳ ($time=2$), $\delta_{trust}=0.1, \alpha=0.1, Score_{new}=0$.
$TrustScore_{new} = 0.5 \times e^{-0.1 \times 2} + 0.1 \times 0 = 0.5 \times e^{-0.2} \approx 0.5 \times 0.8187 \approx 0.409$

**Cân nhắc Thiết kế (Trust Update):**
* **Tham số $\delta_{trust}, \alpha$:** Việc lựa chọn các giá trị này ảnh hưởng lớn đến tốc độ thay đổi uy tín. Nên cho phép DAO điều chỉnh. Cân nhắc `alpha` thay đổi dựa trên mức trust hiện tại.
* **Hàm $f_{update}(Score_{new})$:** Thay vì cộng tuyến tính `alpha * Score_new`, có thể dùng hàm phi tuyến để giới hạn tác động của điểm số đột biến.
* **Liên kết `Score_new`:** Cần làm rõ `Score_new` được lấy chính xác từ đâu ($P_{miner\_adjusted}$ hay nguồn khác).

#### 3.4.2. Xác suất Chọn Miner (Miner Selection Probability)

Tăng cơ hội được chọn cho các Miner ít được chọn gần đây để đảm bảo công bằng.

**Công thức:** [source: 109]
$SelectionProbability = trust\_score \times (1 + \beta \times \min(time\_since\_last\_selection, MaxTimeBonusEffect))$

**Ý nghĩa:** Xác suất chọn cơ bản dựa vào `trust_score`, nhưng được tăng thêm một phần thưởng ($\beta$) tỷ lệ với thời gian kể từ lần cuối được chọn, có giới hạn trên (`MaxTimeBonusEffect`).

**Tham số:** [source: 110]
* $trust\_score$: Điểm tin cậy hiện tại của Miner.
* $\beta$: Hệ số thưởng công bằng (ví dụ: 0.2). *Có thể do DAO quản trị.*
* $time\_since\_last\_selection$: Số chu kỳ/thời gian kể từ lần cuối Miner được chọn.
* $MaxTimeBonusEffect$: Số chu kỳ tối đa mà bonus thời gian có tác dụng (ví dụ: 10). *Ngăn chặn bonus tăng vô hạn.*

**Ví dụ:** [source: 111, 112] (Giả sử không có giới hạn MaxTimeBonusEffect như công thức gốc)
Miner M5 có $trust\_score=0.409$, không được chọn 2 chu kỳ ($time=2$), $\beta=0.2$.
$SelectionProbability = 0.409 \times (1 + 0.2 \times 2) = 0.409 \times 1.4 \approx 0.5726$

**Cân nhắc Thiết kế (Selection Probability):**
* **Giới hạn Bonus:** Việc thêm `MaxTimeBonusEffect` giúp kiểm soát tác động của bonus thời gian.
* **Tham số $\beta$:** Ảnh hưởng đến mức độ ưu tiên cho các miner bị "bỏ quên". Cần được DAO cân nhắc.

### 3.5. Cơ chế Xử lý Vi phạm (Penalty Mechanism)

Các biện pháp để xử phạt hành vi không mong muốn hoặc gian lận.

#### 3.5.1. Điều chỉnh Hiệu suất (Performance Adjustment)

Cho phép Validators phục hồi hiệu suất theo thời gian sau khi bị phạt (không áp dụng trực tiếp cho phạt gian lận).

**Công thức:** [source: 113]
$P_{adjuster\_new} = E_{validator\_new} + \gamma (E_{validator\_base} - E_{validator\_new})$

**Ý nghĩa:** Cho phép điểm hiệu suất của Validator ($E_{validator\_new}$) tăng dần về một mức cơ bản ($E_{validator\_base}$) với tốc độ phục hồi $\gamma$.

**Tham số:** [source: 113]
* $E_{validator\_new}$: Điểm hiệu suất hiện tại.
* $E_{validator\_base}$: Điểm hiệu suất cơ bản (mục tiêu phục hồi).
* $\gamma$: Tỷ lệ phục hồi (ví dụ: 0.1).

**Ví dụ:** [source: 114]
$E_{new}=0.7, E_{base}=0.9, \gamma=0.1$.
$P_{adjuster\_new} = 0.7 + 0.1 \times (0.9 - 0.7) = 0.7 + 0.1 \times 0.2 = 0.72$

#### 3.5.2. Cắt giảm Stake (Stake Slashing)

Giảm lượng stake của người tham gia khi phát hiện gian lận nghiêm trọng.

**Công thức:** [source: 115]
$Slash_{amount} = min(MaxSlashRate \times stake, fraud\_severity \times stake)$

**Ý nghĩa:** Hình phạt tài chính trực tiếp để ngăn chặn hành vi gian lận, mức phạt dựa trên mức độ nghiêm trọng (`fraud_severity`) nhưng không vượt quá một tỷ lệ tối đa (`MaxSlashRate`).

**Tham số:** [source: 115]
* $stake$: Lượng stake hiện tại của người tham gia.
* $fraud\_severity$: Mức độ nghiêm trọng của hành vi gian lận (0 đến 1). *Cần định nghĩa rõ cách xác định.*
* $MaxSlashRate$: Tỷ lệ cắt giảm stake tối đa (ví dụ: 0.2 tức 20%). *Có thể do DAO quản trị.*

**Ví dụ:** [source: 115]
$stake=1000, fraud\_severity=0.15, MaxSlashRate=0.2$.
$Slash_{amount} = min(0.2 \times 1000, 0.15 \times 1000) = min(200, 150) = 150$

**Cân nhắc Thiết kế (Slashing):**
* **Xác định `fraud_severity`:** Cần cơ chế khách quan để xác định mức độ này dựa trên bằng chứng on-chain hoặc off-chain có thể kiểm chứng.
* **`MaxSlashRate`:** Tỷ lệ này cần cân bằng giữa việc đủ sức răn đe và không hủy hoại người tham gia nếu có sai sót trong hệ thống phát hiện gian lận.

#### 3.5.3. Ngưỡng Phát hiện Gian lận (Fraud Detection Threshold)

Cơ chế gắn cờ (flag) hành vi đáng ngờ dựa trên độ lệch đánh giá.

**Logic gốc:** [source: 116] Gắn cờ nếu `|Evaluation - Average Score| / σ > 0.5` trong 3 chu kỳ liên tiếp.

**Ý nghĩa:** Xác định các Validators có hành vi đánh giá bất thường, có thể là dấu hiệu của lỗi hoặc cố ý thao túng.

**Ví dụ:** [source: 116] Nếu điều kiện trên thỏa mãn, `Fraud Flag = 1`.

**Cân nhắc Thiết kế (Fraud Detection):**
* **Ngưỡng cố định:** Ngưỡng `0.5` và `3 chu kỳ` có thể không linh hoạt. Cân nhắc các ngưỡng động hoặc dựa trên nhiều yếu tố hơn, và có thể được DAO điều chỉnh.
* **False Positives:** Cơ chế cần giảm thiểu khả năng phạt nhầm những validator trung thực nhưng có đánh giá khác biệt.

#### 3.5.4. Cập nhật Trust Score do Gian lận

Giảm điểm tin cậy của người tham gia bị gắn cờ gian lận.

**Công thức:** [source: 117]
$trust_{score\_new} = trust_{score\_old} \times (1 - \eta \times Fraud\_Flag)$

**Ý nghĩa:** Giảm uy tín của người tham gia bị phát hiện gian lận, mức độ giảm phụ thuộc vào hệ số phạt $\eta$.

**Tham số:** [source: 118]
* $trust_{score\_old/new}$: Điểm tin cậy cũ/mới.
* $\eta$: Hệ số phạt (ví dụ: 0.1). *Có thể do DAO quản trị.*
* $Fraud\_Flag$: Cờ báo gian lận (0 hoặc 1).

**Ví dụ:** [source: 118]
$trust_{score\_old}=0.9, Fraud\_Flag=1, \eta=0.1$.
$trust_{score\_new} = 0.9 \times (1 - 0.1 \times 1) = 0.9 \times 0.9 = 0.81$

### 3.6. Phân bổ Tài nguyên (Resource Allocation)

Phân phối tài nguyên (ví dụ: băng thông, khả năng tính toán) cho các Subnets.

#### 3.6.1. Phân phối Tài nguyên Subnet ($R_{subnet}$)

Phân bổ dựa trên tổng trọng số và hiệu suất của các thành viên trong Subnet.

**Công thức:** [source: 121]
$R_{subnet} = \frac{\sum_{i \in subnet}(W_i \times P_i)}{\sum_{j \in \text{all subnets}}(W_j \times P_j)} \times R_{total}$

**Ý nghĩa:** Đảm bảo các Subnet đóng góp nhiều giá trị hơn (thông qua trọng số $W$ và hiệu suất $P$) sẽ nhận được phần tài nguyên tương xứng từ tổng tài nguyên $R_{total}$ của hệ thống.

**Tham số:** [source: 121]
* $W_i, P_i$: Trọng số và hiệu suất của thành viên $i$ trong subnet.
* Tử số: Tổng giá trị đóng góp của subnet đang xét.
* Mẫu số: Tổng giá trị đóng góp của toàn bộ hệ thống.
* $R_{total}$: Tổng lượng tài nguyên cần phân bổ.

**Ví dụ:** [source: 122]
Tổng $W \times P$ của Subnet là 30. Tổng $W \times P$ toàn hệ thống là 100. $R_{total}=1000$.
$R_{subnet} = \frac{30}{100} \times 1000 = 300$

### 3.7. Cơ chế Bổ sung: Quản trị DAO (Additional Mechanisms: DAO Governance)

#### 3.7.1. Quyền Biểu quyết DAO (DAO Voting Power)

Xác định sức ảnh hưởng của người tham gia trong việc bỏ phiếu quản trị hệ thống.

**Công thức:** [source: 123]
$VotingPower(p) = stake_p \times (1 + g(\frac{time\_staked_p}{total\_time}) \times Lockup\_Multiplier(p))$

**Ý nghĩa:** Quyền biểu quyết không chỉ dựa trên lượng $stake_p$ mà còn được tăng cường dựa trên thời gian đã stake ($time\_staked_p$, có thể qua hàm phi tuyến $g$) và việc có khóa stake hay không ($Lockup\_Multiplier$). Điều này khuyến khích sự cam kết lâu dài.

**Tham số:** [source: 124, 125]
* $stake_p$: Lượng stake của người tham gia $p$.
* $time\_staked_p$: Thời gian người tham gia $p$ đã stake (cần định nghĩa đơn vị và cách tính $total\_time$).
* $total\_time$: Khoảng thời gian tham chiếu để chuẩn hóa $time\_staked_p$.
* $g(...)$: Hàm phi tuyến (ví dụ: $sqrt$, $log$) để điều chỉnh bonus thời gian (thay vì tuyến tính). *Xem Cân nhắc Thiết kế.*
* $Lockup\_Multiplier(p)$: Hệ số nhân thêm nếu người tham gia $p$ đồng ý khóa stake trong một khoảng thời gian nhất định (ví dụ: = 1 nếu không khóa, > 1 nếu có khóa). *Xem Cân nhắc Thiết kế.*

**Ví dụ:** (Giả sử $g(x)=x$ và không có Lockup như công thức gốc [source: 126, 127])
Người tham gia $p$ có $stake_p=1000$, $time\_staked_p=5$ (năm), $total\_time=10$ (năm).
$VotingPower(p) = 1000 \times (1 + \frac{5}{10}) = 1000 \times 1.5 = 1500$

**Cân nhắc Thiết kế (DAO Voting):**
* **Bonus Thời gian:** Nên dùng hàm phi tuyến $g(x)$ để lợi ích của việc stake lâu dài giảm dần.
* **Cơ chế Lockup:** Triển khai cơ chế khóa stake tự nguyện để tăng quyền biểu quyết có thể tăng tính ổn định và cam kết cho hệ thống.
* **Định nghĩa `total_time`:** Cần làm rõ cách xác định khoảng thời gian tham chiếu này.

## 4. Luồng Hoạt động Hệ thống (System Operation Flow)

Mô tả các bước hoạt động tuần tự của hệ thống Moderntensor: [source: 128-134]

1.  **Giao nhiệm vụ (Task Assignment):** Validators chọn Miners dựa trên `SelectionProbability` (kết hợp trust score và bonus thời gian chờ). [source: 129]
2.  **Thực thi Nhiệm vụ (Task Execution):** Miners được chọn thực hiện công việc (ví dụ: huấn luyện model) và gửi kết quả. [source: 130]
3.  **Đánh giá (Evaluation):** Validators đánh giá kết quả từ Miners, tính ra điểm $P_{miner,v}$. [source: 131]
4.  **Đồng thuận & Hiệu suất Điều chỉnh (Consensus & Adjusted Performance):** Hệ thống tính điểm $P_{miner\_adjusted}$ bằng cách lấy trung bình có trọng số các đánh giá $P_{miner,v}$ dựa trên $trust_{score}$ của Validators. [source: 131]
5.  **Cập nhật Điểm Tin cậy (Trust Score Update):** Cả Trust score của Miners và Validators được cập nhật dựa trên kết quả đánh giá và hoạt động (sử dụng công thức 3.4.1). [source: 132]
6.  **Phân phối Phần thưởng (Reward Distribution):** Phần thưởng được tính và phân phối cho Miners và Validators theo công thức khuyến khích (mục 3.1). [source: 133]
7.  **Cập nhật Blockchain (Blockchain Update):** Các kết quả quan trọng (điểm số, phần thưởng, trạng thái) được ghi lại trên blockchain Cardano (có thể thông qua việc cập nhật Datum trong smart contract). Các dữ liệu lớn có thể được hash và chỉ lưu hash on-chain. [source: 133]
8.  **Quản trị DAO (DAO Governance):** Người tham gia sử dụng Voting Power (tính theo công thức 3.7.1) để bỏ phiếu cho các thay đổi, nâng cấp hoặc điều chỉnh tham số hệ thống. [source: 134]

## 5. Ví dụ Thực tế (Practical Example)

Phần này mô tả một kịch bản cụ thể với 5 Miners và 3 Validators, minh họa từng bước tính toán trong luồng hoạt động hệ thống, từ việc chọn miner, đánh giá, tính hiệu suất điều chỉnh, cập nhật trust score, đến phân phối phần thưởng và tính voting power. [source: 135-141]

*(Giữ nguyên phần ví dụ số trong tài liệu gốc hoặc cập nhật lại nếu các công thức thay đổi đáng kể)*

## 6. Thảo luận Thêm và Hướng Phát triển

* **Tham số Hệ thống:** Nhấn mạnh tầm quan trọng của việc lựa chọn và điều chỉnh các tham số ($\delta, \alpha, \beta, \eta, \lambda, k$, các ngưỡng...). Đề xuất cơ chế quản trị DAO cho các tham số quan trọng.
* **Mô phỏng và Kiểm thử:** Nhấn mạnh sự cần thiết của việc mô phỏng các kịch bản khác nhau (tấn công, tăng trưởng mạng lưới) để kiểm tra sự ổn định và hiệu quả của các cơ chế kinh tế.
* **Khả năng chống Tấn công:** Phân tích sâu hơn về các vector tấn công tiềm ẩn (Sybil, collusion, ...) và đánh giá hiệu quả của các cơ chế phòng chống hiện tại.
* **Vấn đề Cold Start:** Thảo luận về cách người tham gia mới có thể tham gia và xây dựng uy tín hiệu quả trong hệ thống.

---
