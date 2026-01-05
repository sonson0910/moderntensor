# ModernTensor Tokenomics - Implementation Plan

**Ngày:** 5 Tháng 1, 2026  
**Timeline:** Triển khai NGAY sau finalize Layer 1 testnet  
**Target:** Sẵn sàng cho Mainnet Q1 2026

---

## 🎯 Executive Summary

**Kế hoạch triển khai tokenomics adaptive cho ModernTensor, vượt trội so với Bittensor's fixed emission model.**

**Key Differences vs Bittensor:**
- Bittensor: Fixed emission (in cố định mỗi block bất kể nhu cầu)
- ModernTensor: **Adaptive emission** (emission thay đổi theo utility + recycling pool)

**Timeline:** 6-8 tuần implementation sau testnet finalization

---

## 📋 Phase 1: Core Tokenomics Smart Contracts (Weeks 1-3)

### 1.1 Emission Controller Contract

**File:** `sdk/tokenomics/emission_controller.py`

**Chức năng chính:**

```python
class EmissionController:
    """
    Quản lý adaptive emission dựa trên network utility.
    
    Core Formula:
        MintAmount = BaseReward × UtilityScore × EmissionMultiplier
    
    Where:
        - BaseReward: Base reward per epoch (decreases over time)
        - UtilityScore: 0.0 to 1.0 based on network activity
        - EmissionMultiplier: Halving schedule
    """
    
    def __init__(self, config: TokenomicsConfig):
        self.config = config
        self.current_supply = 0
        self.max_supply = 21_000_000  # 21M tokens
        self.base_reward = 1000  # Initial reward
        
    def calculate_epoch_emission(
        self,
        utility_score: float,
        epoch: int
    ) -> int:
        """
        Calculate emission for current epoch.
        
        Args:
            utility_score: Network utility (0.0 to 1.0)
            epoch: Current epoch number
            
        Returns:
            Token amount to mint this epoch
        """
        # Halving schedule (every 210,000 epochs ~ 4 years at 10min/epoch)
        halvings = epoch // 210000
        emission_multiplier = 0.5 ** halvings
        
        # Adaptive emission based on utility
        mint_amount = (
            self.base_reward * 
            utility_score * 
            emission_multiplier
        )
        
        # Cap at max supply
        if self.current_supply + mint_amount > self.max_supply:
            mint_amount = self.max_supply - self.current_supply
            
        return int(mint_amount)
    
    def calculate_utility_score(
        self,
        task_volume: int,
        avg_task_difficulty: float,
        validator_participation: float
    ) -> float:
        """
        Calculate network utility score.
        
        Formula:
            U = w1 × TaskVolumeScore + 
                w2 × DifficultyScore + 
                w3 × ParticipationScore
        
        Where w1 + w2 + w3 = 1.0
        """
        # Weights (configurable)
        w1, w2, w3 = 0.5, 0.3, 0.2
        
        # Normalize task volume (0-1 scale)
        max_tasks = self.config.max_expected_tasks
        task_score = min(task_volume / max_tasks, 1.0)
        
        # Difficulty already 0-1
        difficulty_score = avg_task_difficulty
        
        # Participation already 0-1
        participation_score = validator_participation
        
        utility = (
            w1 * task_score +
            w2 * difficulty_score +
            w3 * participation_score
        )
        
        return min(utility, 1.0)
```

**Implementation Steps:**

**Week 1:**
- [ ] Design EmissionController class
- [ ] Implement base emission calculation
- [ ] Implement halving schedule
- [ ] Unit tests (target: 20 tests)

**Week 2:**
- [ ] Implement utility score calculation
- [ ] Add task volume tracking
- [ ] Add difficulty measurement
- [ ] Add validator participation tracking
- [ ] Integration tests

**Week 3:**
- [ ] Add supply cap enforcement
- [ ] Add emission history tracking
- [ ] Add statistics/metrics
- [ ] Documentation
- [ ] Code review & security audit

---

### 1.2 Recycling Pool Contract

**File:** `sdk/tokenomics/recycling_pool.py`

**Chức năng chính:**

```python
class RecyclingPool:
    """
    Tái chế tokens từ fees, slashing, penalties.
    Ưu tiên dùng recycled tokens trước khi mint new tokens.
    """
    
    def __init__(self):
        self.pool_balance = 0
        self.sources = {
            'registration_fees': 0,
            'slashing_penalties': 0,
            'task_fees': 0,
            'transaction_fees': 0
        }
        
    def add_to_pool(
        self,
        amount: int,
        source: str
    ):
        """Add tokens to recycling pool."""
        self.pool_balance += amount
        self.sources[source] += amount
        
    def allocate_rewards(
        self,
        required_amount: int,
        emission_controller: EmissionController
    ) -> tuple[int, int]:
        """
        Allocate rewards from pool or minting.
        
        Args:
            required_amount: Total rewards needed
            emission_controller: For minting if pool insufficient
            
        Returns:
            (from_pool, from_mint)
        """
        # Try to use recycled tokens first
        from_pool = min(self.pool_balance, required_amount)
        from_mint = required_amount - from_pool
        
        # Deduct from pool
        if from_pool > 0:
            self.pool_balance -= from_pool
            
        # Mint additional if needed
        if from_mint > 0:
            # Check with emission controller
            max_mintable = emission_controller.calculate_epoch_emission(...)
            from_mint = min(from_mint, max_mintable)
            
        return (from_pool, from_mint)
    
    def get_pool_stats(self) -> dict:
        """Get pool statistics."""
        return {
            'total_balance': self.pool_balance,
            'sources': self.sources.copy(),
            'utilization_rate': self._calculate_utilization()
        }
```

**Implementation Steps:**

**Week 2:**
- [ ] Design RecyclingPool class
- [ ] Implement pool balance tracking
- [ ] Implement source tracking (fees, slashing, etc.)
- [ ] Unit tests

**Week 3:**
- [ ] Implement reward allocation logic
- [ ] Integration with EmissionController
- [ ] Add pool statistics
- [ ] Testing & documentation

---

### 1.3 Burn Mechanism

**File:** `sdk/tokenomics/burn_manager.py`

**Chức năng chính:**

```python
class BurnManager:
    """
    Quản lý token burning mechanisms.
    """
    
    def __init__(self):
        self.total_burned = 0
        self.burn_reasons = {
            'unmet_quota': 0,
            'transaction_fees': 0,
            'quality_penalty': 0
        }
        
    def burn_unmet_quota(
        self,
        expected_emission: int,
        actual_quality_score: float,
        threshold: float = 0.5
    ) -> int:
        """
        Burn tokens if network quality doesn't meet threshold.
        
        Args:
            expected_emission: Emission planned for epoch
            actual_quality_score: 0.0 to 1.0
            threshold: Minimum quality threshold
            
        Returns:
            Amount burned
        """
        if actual_quality_score < threshold:
            # Burn proportional to quality deficit
            burn_amount = int(
                expected_emission * (threshold - actual_quality_score)
            )
            self._burn(burn_amount, 'unmet_quota')
            return burn_amount
        return 0
    
    def burn_transaction_fees(
        self,
        fee_amount: int,
        burn_percentage: float = 0.5
    ) -> int:
        """
        Burn percentage of transaction fees.
        
        Args:
            fee_amount: Total fees collected
            burn_percentage: Percentage to burn (default 50%)
            
        Returns:
            Amount burned
        """
        burn_amount = int(fee_amount * burn_percentage)
        self._burn(burn_amount, 'transaction_fees')
        return burn_amount
    
    def _burn(self, amount: int, reason: str):
        """Internal burn function."""
        self.total_burned += amount
        self.burn_reasons[reason] += amount
```

**Implementation Steps:**

**Week 3:**
- [ ] Design BurnManager class
- [ ] Implement unmet quota burn
- [ ] Implement transaction fee burn
- [ ] Add burn tracking and statistics
- [ ] Unit tests
- [ ] Integration with EmissionController

---

## 📋 Phase 2: Reward Distribution System (Weeks 3-4)

### 2.1 Reward Distributor

**File:** `sdk/tokenomics/reward_distributor.py`

**Chức năng chính:**

```python
class RewardDistributor:
    """
    Phân phối rewards cho Miners, Validators, và DAO.
    """
    
    def __init__(self, config: DistributionConfig):
        self.config = config
        # Default split: 40% Miners, 40% Validators, 20% DAO
        self.miner_share = 0.40
        self.validator_share = 0.40
        self.dao_share = 0.20
        
    def distribute_epoch_rewards(
        self,
        total_emission: int,
        miner_scores: Dict[str, float],
        validator_stakes: Dict[str, int],
        recycling_pool: RecyclingPool
    ) -> DistributionResult:
        """
        Distribute rewards for an epoch.
        
        Args:
            total_emission: Total tokens to distribute
            miner_scores: Miner UID -> performance score
            validator_stakes: Validator address -> stake amount
            recycling_pool: For token sourcing
            
        Returns:
            DistributionResult with details
        """
        # Get tokens from pool or mint
        from_pool, from_mint = recycling_pool.allocate_rewards(
            total_emission, emission_controller
        )
        
        # Split into pools
        miner_pool = int(total_emission * self.miner_share)
        validator_pool = int(total_emission * self.validator_share)
        dao_pool = int(total_emission * self.dao_share)
        
        # Distribute to miners (by performance)
        miner_rewards = self._distribute_to_miners(
            miner_pool, miner_scores
        )
        
        # Distribute to validators (by stake)
        validator_rewards = self._distribute_to_validators(
            validator_pool, validator_stakes
        )
        
        return DistributionResult(
            epoch=epoch,
            total_distributed=total_emission,
            from_pool=from_pool,
            from_mint=from_mint,
            miner_rewards=miner_rewards,
            validator_rewards=validator_rewards,
            dao_allocation=dao_pool
        )
    
    def _distribute_to_miners(
        self,
        pool: int,
        scores: Dict[str, float]
    ) -> Dict[str, int]:
        """Distribute pool to miners proportional to scores."""
        total_score = sum(scores.values())
        if total_score == 0:
            return {}
            
        rewards = {}
        for uid, score in scores.items():
            reward = int((score / total_score) * pool)
            rewards[uid] = reward
            
        return rewards
    
    def _distribute_to_validators(
        self,
        pool: int,
        stakes: Dict[str, int]
    ) -> Dict[str, int]:
        """Distribute pool to validators proportional to stake."""
        total_stake = sum(stakes.values())
        if total_stake == 0:
            return {}
            
        rewards = {}
        for address, stake in stakes.items():
            reward = int((stake / total_stake) * pool)
            rewards[address] = reward
            
        return rewards
```

**Implementation Steps:**

**Week 4:**
- [ ] Design RewardDistributor class
- [ ] Implement 3-way split (Miners/Validators/DAO)
- [ ] Implement miner distribution (by performance)
- [ ] Implement validator distribution (by stake)
- [ ] Add DAO treasury management
- [ ] Unit tests (30+ tests)
- [ ] Integration tests

---

### 2.2 Claim System

**File:** `sdk/tokenomics/claim_manager.py`

**Chức năng chính:**

```python
class ClaimManager:
    """
    Quản lý claims cho rewards với Merkle proofs.
    """
    
    def __init__(self):
        self.pending_claims = {}  # epoch -> ClaimData
        self.claimed = {}         # (epoch, address) -> amount
        
    def create_claim_tree(
        self,
        epoch: int,
        rewards: Dict[str, int]
    ) -> bytes:
        """
        Create Merkle tree for claims.
        
        Args:
            epoch: Epoch number
            rewards: Address -> reward amount
            
        Returns:
            Merkle root
        """
        from sdk.utils.merkle_tree import MerkleTree
        
        # Create leaves (address, amount) pairs
        leaves = []
        for address, amount in sorted(rewards.items()):
            leaf = hashlib.sha256(
                address.encode() + amount.to_bytes(32, 'big')
            ).digest()
            leaves.append(leaf)
            
        # Build tree
        tree = MerkleTree(leaves)
        root = tree.get_root()
        
        # Store for later verification
        self.pending_claims[epoch] = {
            'root': root,
            'tree': tree,
            'rewards': rewards
        }
        
        return root
    
    def claim_reward(
        self,
        epoch: int,
        address: str,
        amount: int,
        proof: List[bytes]
    ) -> bool:
        """
        Claim reward with Merkle proof.
        
        Args:
            epoch: Epoch number
            address: Claimer address
            amount: Claimed amount
            proof: Merkle proof
            
        Returns:
            True if claim successful
        """
        # Check if already claimed
        if (epoch, address) in self.claimed:
            raise ValueError("Already claimed")
            
        # Verify proof
        claim_data = self.pending_claims.get(epoch)
        if not claim_data:
            raise ValueError("Invalid epoch")
            
        leaf = hashlib.sha256(
            address.encode() + amount.to_bytes(32, 'big')
        ).digest()
        
        if not claim_data['tree'].verify_proof(
            leaf, proof, claim_data['root']
        ):
            raise ValueError("Invalid proof")
            
        # Mark as claimed
        self.claimed[(epoch, address)] = amount
        
        return True
```

**Implementation Steps:**

**Week 4:**
- [ ] Design ClaimManager class
- [ ] Implement Merkle tree creation for claims
- [ ] Implement claim verification with proofs
- [ ] Add double-claim prevention
- [ ] Unit tests
- [ ] Integration with RewardDistributor

---

## 📋 Phase 3: Integration with Layer 1 (Weeks 5-6)

### 3.1 Tokenomics Integration Module

**File:** `sdk/tokenomics/integration.py`

**Chức năng chính:**

```python
class TokenomicsIntegration:
    """
    Tích hợp tokenomics với Layer 1 blockchain.
    """
    
    def __init__(
        self,
        emission_controller: EmissionController,
        recycling_pool: RecyclingPool,
        reward_distributor: RewardDistributor,
        burn_manager: BurnManager,
        claim_manager: ClaimManager
    ):
        self.emission = emission_controller
        self.pool = recycling_pool
        self.distributor = reward_distributor
        self.burn = burn_manager
        self.claims = claim_manager
        
    def process_epoch_tokenomics(
        self,
        epoch: int,
        consensus_data: ConsensusData,
        network_metrics: NetworkMetrics
    ) -> EpochTokenomics:
        """
        Process complete tokenomics for an epoch.
        
        This is called at the end of each consensus epoch.
        
        Steps:
        1. Calculate utility score
        2. Calculate emission amount
        3. Check recycling pool
        4. Distribute rewards
        5. Handle burns
        6. Create claim tree
        7. Update state
        """
        # 1. Calculate utility
        utility = self.emission.calculate_utility_score(
            task_volume=network_metrics.task_count,
            avg_task_difficulty=network_metrics.avg_difficulty,
            validator_participation=network_metrics.validator_ratio
        )
        
        # 2. Calculate emission
        epoch_emission = self.emission.calculate_epoch_emission(
            utility_score=utility,
            epoch=epoch
        )
        
        # 3. Distribute rewards
        distribution = self.distributor.distribute_epoch_rewards(
            total_emission=epoch_emission,
            miner_scores=consensus_data.miner_scores,
            validator_stakes=consensus_data.validator_stakes,
            recycling_pool=self.pool
        )
        
        # 4. Handle burns (if network quality poor)
        burned = self.burn.burn_unmet_quota(
            expected_emission=epoch_emission,
            actual_quality_score=consensus_data.quality_score
        )
        
        # 5. Create claim tree
        all_rewards = {
            **distribution.miner_rewards,
            **distribution.validator_rewards
        }
        claim_root = self.claims.create_claim_tree(
            epoch=epoch,
            rewards=all_rewards
        )
        
        # 6. Update SubnetAggregatedDatum
        return EpochTokenomics(
            epoch=epoch,
            utility_score=utility,
            emission_amount=epoch_emission,
            burned_amount=burned,
            miner_pool=sum(distribution.miner_rewards.values()),
            validator_pool=sum(distribution.validator_rewards.values()),
            dao_allocation=distribution.dao_allocation,
            claim_root=claim_root,
            from_pool=distribution.from_pool,
            from_mint=distribution.from_mint
        )
```

**Implementation Steps:**

**Week 5:**
- [ ] Design TokenomicsIntegration class
- [ ] Implement epoch processing workflow
- [ ] Connect with Layer1ConsensusIntegrator
- [ ] Connect with SubnetAggregatedDatum updates
- [ ] Unit tests

**Week 6:**
- [ ] Integration testing with full blockchain
- [ ] End-to-end epoch simulation
- [ ] Performance testing
- [ ] Bug fixes

---

### 3.2 Update SubnetAggregatedDatum

**File:** `sdk/metagraph/aggregated_state.py` (modify existing)

**Add tokenomics fields:**

```python
@dataclass
class SubnetAggregatedDatum(PlutusData):
    # ... existing fields ...
    
    # NEW: Tokenomics fields
    utility_score_scaled: int          # Current utility score (scaled)
    epoch_emission: int                # Emission this epoch
    total_burned: int                  # Total burned to date
    recycling_pool_balance: int        # Current pool balance
    claim_root: bytes                  # Merkle root for claims (32 bytes)
    
    # Emission breakdown
    miner_pool_this_epoch: int
    validator_pool_this_epoch: int
    dao_allocation_this_epoch: int
    
    @property
    def utility_score(self) -> float:
        return self.utility_score_scaled / DATUM_INT_DIVISOR
```

**Implementation Steps:**

**Week 5:**
- [ ] Add new fields to SubnetAggregatedDatum
- [ ] Update serialization/deserialization
- [ ] Update existing code to handle new fields
- [ ] Migration script for existing data
- [ ] Tests

---

## 📋 Phase 4: Network Metrics Collection (Week 6-7)

### 4.1 Metrics Collector

**File:** `sdk/tokenomics/metrics_collector.py`

**Chức năng chính:**

```python
class NetworkMetricsCollector:
    """
    Thu thập metrics cho utility score calculation.
    """
    
    def __init__(self):
        self.current_epoch_metrics = NetworkMetrics()
        
    def record_task_submission(
        self,
        task: AITask,
        difficulty: float
    ):
        """Record AI task submission."""
        self.current_epoch_metrics.task_count += 1
        self.current_epoch_metrics.total_difficulty += difficulty
        
    def record_validator_participation(
        self,
        validator: str,
        participated: bool
    ):
        """Record validator participation in consensus."""
        if participated:
            self.current_epoch_metrics.active_validators += 1
            
    def get_epoch_metrics(self) -> NetworkMetrics:
        """Get metrics for current epoch."""
        metrics = self.current_epoch_metrics
        
        # Calculate averages
        if metrics.task_count > 0:
            metrics.avg_difficulty = (
                metrics.total_difficulty / metrics.task_count
            )
        
        if metrics.total_validators > 0:
            metrics.validator_ratio = (
                metrics.active_validators / metrics.total_validators
            )
            
        return metrics
    
    def reset_for_new_epoch(self):
        """Reset metrics for new epoch."""
        self.current_epoch_metrics = NetworkMetrics()
```

**Implementation Steps:**

**Week 6:**
- [ ] Design NetworkMetricsCollector
- [ ] Integrate with AI task system
- [ ] Integrate with consensus system
- [ ] Add metrics persistence
- [ ] Unit tests

**Week 7:**
- [ ] Connect to TokenomicsIntegration
- [ ] Add metrics dashboard/API
- [ ] Testing & optimization

---

## 📋 Phase 5: Testing & Documentation (Week 7-8)

### 5.1 Comprehensive Testing

**Test Coverage Goals:**
- Unit tests: 150+ tests
- Integration tests: 30+ tests
- End-to-end tests: 10+ scenarios

**Test Scenarios:**

```python
# tests/tokenomics/test_full_cycle.py

async def test_full_tokenomics_cycle():
    """Test complete tokenomics cycle from task to claim."""
    
    # 1. Setup network
    network = create_test_network(miners=10, validators=5)
    
    # 2. Submit tasks
    for i in range(100):
        task = create_ai_task()
        network.submit_task(task)
        
    # 3. Run consensus
    consensus_result = await network.run_consensus_round()
    
    # 4. Process tokenomics
    tokenomics_result = network.process_epoch_tokenomics()
    
    # 5. Verify emission calculation
    assert tokenomics_result.utility_score > 0
    assert tokenomics_result.emission_amount > 0
    
    # 6. Verify distribution
    total_distributed = (
        sum(tokenomics_result.miner_rewards.values()) +
        sum(tokenomics_result.validator_rewards.values()) +
        tokenomics_result.dao_allocation
    )
    assert total_distributed == tokenomics_result.emission_amount
    
    # 7. Verify claims
    for miner, reward in tokenomics_result.miner_rewards.items():
        proof = network.get_claim_proof(miner)
        assert network.verify_claim(miner, reward, proof)
        
    # 8. Test double-claim prevention
    with pytest.raises(ValueError):
        network.claim_reward(miner, reward, proof)
```

**Implementation Steps:**

**Week 7:**
- [ ] Write unit tests for all components
- [ ] Write integration tests
- [ ] Write end-to-end tests
- [ ] Run test coverage analysis (target: >90%)

**Week 8:**
- [ ] Bug fixes from testing
- [ ] Performance optimization
- [ ] Security review
- [ ] Final testing round

---

### 5.2 Documentation

**Documentation Deliverables:**

1. **Technical Specification** (`docs/TOKENOMICS_SPEC.md`)
   - Complete formulas
   - Data structures
   - API reference
   
2. **User Guide** (`docs/TOKENOMICS_USER_GUIDE.md`)
   - How to earn rewards
   - How to claim rewards
   - FAQ
   
3. **Integration Guide** (`docs/TOKENOMICS_INTEGRATION.md`)
   - How to integrate with dApps
   - API examples
   - Best practices

**Implementation Steps:**

**Week 8:**
- [ ] Write technical specification
- [ ] Write user guide
- [ ] Write integration guide
- [ ] Create diagrams and visualizations
- [ ] Review and publish

---

## 📋 Phase 6: Deployment to Testnet (Week 8)

### 6.1 Testnet Deployment Plan

**Steps:**

1. **Deploy tokenomics contracts**
   ```bash
   python deploy_tokenomics.py --network testnet
   ```

2. **Initialize with test parameters**
   ```python
   config = TokenomicsConfig(
       max_supply=21_000_000,
       base_reward=1000,
       halving_interval=210000,
       miner_share=0.40,
       validator_share=0.40,
       dao_share=0.20
   )
   ```

3. **Run validation tests**
   - Submit test tasks
   - Trigger consensus
   - Verify emission calculation
   - Test claim mechanism

4. **Monitor initial epochs**
   - Track utility scores
   - Monitor emission amounts
   - Check recycling pool
   - Verify distributions

**Week 8:**
- [ ] Deploy to testnet
- [ ] Run validation tests
- [ ] Monitor for 1 week
- [ ] Collect feedback
- [ ] Bug fixes if needed
- [ ] Prepare for mainnet

---

## 📋 Timeline Summary

| Phase | Duration | Tasks | Status |
|-------|----------|-------|--------|
| **Phase 1:** Core Contracts | Weeks 1-3 | Emission, Recycling, Burn | ⏸️ Todo |
| **Phase 2:** Reward System | Weeks 3-4 | Distributor, Claims | ⏸️ Todo |
| **Phase 3:** L1 Integration | Weeks 5-6 | Integration module | ⏸️ Todo |
| **Phase 4:** Metrics | Weeks 6-7 | Collector, Dashboard | ⏸️ Todo |
| **Phase 5:** Testing | Weeks 7-8 | Full test suite | ⏸️ Todo |
| **Phase 6:** Deployment | Week 8 | Testnet launch | ⏸️ Todo |

**Total Time:** 8 weeks  
**Start:** Ngay sau finalize Layer 1 testnet  
**Completion:** Ready for Mainnet

---

## 🎯 Success Criteria

### Technical Metrics:
- [ ] 150+ unit tests passing
- [ ] 30+ integration tests passing
- [ ] 10+ end-to-end scenarios working
- [ ] >90% code coverage
- [ ] Zero critical security issues

### Functional Metrics:
- [ ] Emission calculation accurate
- [ ] Utility score reflects network activity
- [ ] Recycling pool working correctly
- [ ] Burn mechanism activates when needed
- [ ] Claims work with Merkle proofs
- [ ] DAO treasury receiving allocation

### Performance Metrics:
- [ ] Epoch processing < 10 seconds
- [ ] Claim verification < 1 second
- [ ] Metrics collection minimal overhead
- [ ] State updates efficient

---

## 🚀 Deployment Checklist

### Pre-Deployment:
- [ ] All tests passing
- [ ] Security audit complete
- [ ] Documentation complete
- [ ] Testnet validation successful
- [ ] Community review done

### Deployment:
- [ ] Deploy tokenomics contracts
- [ ] Initialize parameters
- [ ] Connect with consensus
- [ ] Activate metrics collection
- [ ] Enable claims system

### Post-Deployment:
- [ ] Monitor first 10 epochs
- [ ] Track metrics dashboard
- [ ] Collect user feedback
- [ ] Bug triage and fixes
- [ ] Performance optimization

---

## 💰 Budget Estimate

| Category | Cost (USD) | Notes |
|----------|-----------|-------|
| Development (8 weeks) | $80,000 | 2 engineers × $40k |
| Security Audit | $30,000 | External audit |
| Testing Infrastructure | $5,000 | Testnet costs |
| Documentation | $5,000 | Technical writer |
| **Total** | **$120,000** | |

---

## 📞 Conclusion

**Kế hoạch này cung cấp:**

1. ✅ **Complete tokenomics** tốt hơn Bittensor's fixed model
2. ✅ **Adaptive emission** based on utility
3. ✅ **Recycling mechanism** giảm inflation
4. ✅ **Burn mechanism** để giữ giá trị token
5. ✅ **Fair distribution** cho miners, validators, DAO
6. ✅ **Merkle proofs** cho efficient claims
7. ✅ **Full testing** và security
8. ✅ **8 weeks timeline** từ start đến testnet

**Next Steps:**
1. Review và approve plan
2. Allocate budget
3. Start implementation ngay sau testnet finalization
4. Target: Ready for Mainnet Q1 2026

---

**Prepared by:** GitHub Copilot  
**Date:** January 5, 2026  
**Status:** ⏸️ Ready to Implement
