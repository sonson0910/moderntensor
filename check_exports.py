"""Check consensus module exports"""
import sys
sys.path.insert(0, '.')

modules = [
    ('pos', ['ProofOfStake', 'ConsensusConfig', 'ValidatorInfo']),
    ('fork_choice', ['ForkChoice', 'BlockInfo']),
    ('liveness', ['LivenessMonitor', 'LivenessConfig', 'LivenessStats']),
    ('rotation', ['ValidatorRotation', 'RotationConfig']),
    ('slashing', ['SlashingManager', 'SlashingConfig', 'SlashEvent']),
    ('halving', ['HalvingSchedule', 'HalvingConfig']),
    ('long_range_protection', ['LongRangeProtection', 'LongRangeConfig', 'Checkpoint']),
    ('fast_finality', ['FastFinality', 'FastFinalityConfig']),
    ('circuit_breaker', ['CircuitBreaker', 'CircuitBreakerConfig', 'CircuitBreakerOp']),
    ('fork_resolution', ['ForkResolver', 'ForkResolutionConfig', 'ReorgInfo']),
]

for module_name, expected_exports in modules:
    try:
        module = __import__(f'sdk.consensus.{module_name}', fromlist=expected_exports)
        for export in expected_exports:
            if hasattr(module, export):
                print(f"✅ {module_name}.{export}")
            else:
                print(f"❌ {module_name}.{export} NOT FOUND")
                print(f"   Available: {[x for x in dir(module) if not x.startswith('_')]}")
    except Exception as e:
        print(f"❌ {module_name}: {e}")
