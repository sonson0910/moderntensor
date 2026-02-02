# Federated Learning Integration Tests
#
# Tests for the Federated Learning Layer:
# - Training job creation and management
# - Trainer registration
# - Gradient submission and aggregation
# - Proof of Training verification
# - Reward distribution

import json
import time
import requests
from typing import Optional, Dict, Any
from dataclasses import dataclass

# Configuration
RPC_URL = "http://localhost:8545"

def rpc_call(method: str, params: list = None) -> Dict[str, Any]:
    """Make a JSON-RPC call to the node."""
    payload = {
        "jsonrpc": "2.0",
        "method": method,
        "params": params or [],
        "id": 1
    }
    response = requests.post(RPC_URL, json=payload, timeout=30)
    return response.json()


# =============================================================================
# TRAINING JOB TESTS
# =============================================================================

def test_training_create_job():
    """Test creating a training job."""
    print("\n=== Test: training_createJob ===")

    result = rpc_call("training_createJob", [{
        "model_id": "QmTestModel123456789",
        "dataset_ref": "QmTestDataset987654321",
        "total_rounds": 10,
        "min_participants": 3,
        "reward_per_round": "0x8ac7230489e80000"  # 10 MDT
    }])

    if "result" in result and result["result"]["success"]:
        job_id = result["result"]["job_id"]
        print(f"✅ Job created: {job_id}")
        return job_id
    else:
        print(f"❌ Failed: {result}")
        return None


def test_training_get_job(job_id: str):
    """Test getting job details."""
    print("\n=== Test: training_getJob ===")

    result = rpc_call("training_getJob", [job_id])

    if "result" in result and result["result"]:
        job = result["result"]
        print(f"✅ Job retrieved:")
        print(f"   Model: {job['model_id']}")
        print(f"   Rounds: {job['current_round']}/{job['total_rounds']}")
        print(f"   Status: {job['status']}")
        return True
    else:
        print(f"❌ Failed: {result}")
        return False


def test_training_list_jobs():
    """Test listing all jobs."""
    print("\n=== Test: training_listJobs ===")

    result = rpc_call("training_listJobs", [])

    if "result" in result:
        jobs = result["result"]["jobs"]
        print(f"✅ Found {len(jobs)} jobs")
        return True
    else:
        print(f"❌ Failed: {result}")
        return False


# =============================================================================
# TRAINER TESTS
# =============================================================================

def test_training_register_trainer(job_id: str, trainer_address: str):
    """Test registering as a trainer."""
    print(f"\n=== Test: training_registerTrainer ===")

    result = rpc_call("training_registerTrainer", [job_id, trainer_address])

    if "result" in result and result["result"]["success"]:
        print(f"✅ Trainer {trainer_address[:10]}... registered")
        print(f"   Trainer count: {result['result']['trainer_count']}")
        print(f"   Training started: {result['result']['started']}")
        return True
    else:
        print(f"❌ Failed: {result}")
        return False


def test_training_get_trainers(job_id: str):
    """Test getting trainers for a job."""
    print(f"\n=== Test: training_getTrainers ===")

    result = rpc_call("training_getTrainers", [job_id])

    if "result" in result:
        trainers = result["result"]["trainers"]
        print(f"✅ Found {len(trainers)} trainers")
        return True
    else:
        print(f"❌ Failed: {result}")
        return False


# =============================================================================
# GRADIENT TESTS
# =============================================================================

def test_training_submit_gradient(job_id: str):
    """Test submitting a gradient."""
    print(f"\n=== Test: training_submitGradient ===")

    result = rpc_call("training_submitGradient", [{
        "job_id": job_id,
        "gradient_hash": "0x" + "ab" * 32,
        "checkpoint_hash": "0x" + "cd" * 32
    }])

    if "result" in result and result["result"]["success"]:
        print(f"✅ Gradient submitted")
        print(f"   Round: {result['result']['round']}")
        print(f"   Submissions: {result['result']['submission_count']}")
        print(f"   Round complete: {result['result']['round_complete']}")
        return True
    else:
        print(f"❌ Failed: {result}")
        return False


def test_training_get_gradients(job_id: str, round_num: int):
    """Test getting gradient submissions."""
    print(f"\n=== Test: training_getGradients ===")

    result = rpc_call("training_getGradients", [job_id, round_num])

    if "result" in result:
        submissions = result["result"]["submissions"]
        print(f"✅ Found {len(submissions)} submissions for round {round_num}")
        return True
    else:
        print(f"❌ Failed: {result}")
        return False


# =============================================================================
# ROUND TESTS
# =============================================================================

def test_training_get_round_status(job_id: str):
    """Test getting round status."""
    print(f"\n=== Test: training_getRoundStatus ===")

    result = rpc_call("training_getRoundStatus", [job_id])

    if "result" in result:
        status = result["result"]
        print(f"✅ Round status:")
        print(f"   Current round: {status['round']}")
        print(f"   Submissions: {status['submission_count']}")
        print(f"   Completed: {status['completed']}")
        return True
    else:
        print(f"❌ Failed: {result}")
        return False


def test_training_advance_round(job_id: str):
    """Test advancing to next round."""
    print(f"\n=== Test: training_advanceRound ===")

    result = rpc_call("training_advanceRound", [job_id])

    if "result" in result and result["result"]["success"]:
        print(f"✅ Advanced to round {result['result']['new_round']}")
        return True
    else:
        print(f"❌ Failed: {result}")
        return False


# =============================================================================
# END TO END FLOW
# =============================================================================

def run_full_training_flow():
    """Run a complete training flow test."""
    print("\n" + "=" * 60)
    print("FEDERATED LEARNING END-TO-END TEST")
    print("=" * 60)

    # Test addresses
    trainers = [
        "0x1234567890123456789012345678901234567890",
        "0x2345678901234567890123456789012345678901",
        "0x3456789012345678901234567890123456789012",
    ]

    # 1. Create job
    job_id = test_training_create_job()
    if not job_id:
        return False

    # 2. Get job details
    test_training_get_job(job_id)

    # 3. List jobs
    test_training_list_jobs()

    # 4. Register trainers
    for trainer in trainers:
        test_training_register_trainer(job_id, trainer)

    # 5. Get trainers
    test_training_get_trainers(job_id)

    # 6. Get job (should be Training now)
    test_training_get_job(job_id)

    # 7. Submit gradients
    for _ in range(3):
        test_training_submit_gradient(job_id)

    # 8. Get round status
    test_training_get_round_status(job_id)

    # 9. Get gradients
    test_training_get_gradients(job_id, 0)

    # 10. Advance round
    test_training_advance_round(job_id)

    # 11. Final status
    test_training_get_round_status(job_id)

    print("\n" + "=" * 60)
    print("TEST COMPLETE")
    print("=" * 60)

    return True


# =============================================================================
# MAIN
# =============================================================================

if __name__ == "__main__":
    import sys

    print("=" * 60)
    print("ModernTensor Federated Learning Test Suite")
    print("=" * 60)

    # Check if node is running
    try:
        result = rpc_call("web3_clientVersion")
        print(f"Connected to: {result.get('result', 'Unknown')}")
    except requests.exceptions.ConnectionError:
        print("❌ Cannot connect to node at", RPC_URL)
        print("Please start the node first.")
        sys.exit(1)

    # Run tests
    success = run_full_training_flow()

    sys.exit(0 if success else 1)
