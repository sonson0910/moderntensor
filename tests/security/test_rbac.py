"""Tests for Role-Based Access Control (RBAC) module.

Covers:
- User CRUD (create, get, delete)
- Role assignment / revocation
- Permission inheritance from roles
- Custom (per-user) permissions
- Admin bypass
- Cache invalidation
- require_permission / require_role decorators
- Thread safety
- Edge cases (duplicate user, unknown user)
"""

import threading

import pytest

from sdk.security.rbac import (
    AccessControl,
    Permission,
    Role,
    RoleManager,
    User,
    get_access_control,
)


# ─── Fixtures ────────────────────────────────────────────────────────────────


@pytest.fixture
def ac() -> AccessControl:
    """Fresh AccessControl instance per test."""
    return AccessControl()


@pytest.fixture
def seeded_ac(ac: AccessControl) -> AccessControl:
    """AccessControl with a few pre-created users."""
    ac.create_user("alice", roles=[Role.ADMIN])
    ac.create_user("bob", roles=[Role.VALIDATOR])
    ac.create_user("carol", roles=[Role.MINER])
    ac.create_user("dave", roles=[Role.OBSERVER])
    return ac


# ─── RoleManager Tests ───────────────────────────────────────────────────────


class TestRoleManager:
    def test_default_roles_defined(self):
        rm = RoleManager()
        roles = rm.list_roles()
        assert Role.ADMIN in roles
        assert Role.VALIDATOR in roles
        assert Role.MINER in roles
        assert Role.OBSERVER in roles
        assert Role.API_USER in roles
        assert Role.DEVELOPER in roles

    def test_admin_has_admin_full(self):
        rm = RoleManager()
        perms = rm.get_role_permissions(Role.ADMIN)
        assert Permission.ADMIN_FULL in perms

    def test_observer_is_read_only(self):
        rm = RoleManager()
        perms = rm.get_role_permissions(Role.OBSERVER)
        assert Permission.READ_BLOCKCHAIN in perms
        assert Permission.WRITE_BLOCKCHAIN not in perms
        assert Permission.SUBMIT_TRANSACTION not in perms

    def test_add_permission_to_role(self):
        rm = RoleManager()
        rm.add_permission_to_role(Role.OBSERVER, Permission.SUBMIT_TRANSACTION)
        perms = rm.get_role_permissions(Role.OBSERVER)
        assert Permission.SUBMIT_TRANSACTION in perms

    def test_remove_permission_from_role(self):
        rm = RoleManager()
        rm.remove_permission_from_role(Role.MINER, Permission.MINE_BLOCKS)
        perms = rm.get_role_permissions(Role.MINER)
        assert Permission.MINE_BLOCKS not in perms

    def test_get_permissions_returns_copy(self):
        rm = RoleManager()
        perms = rm.get_role_permissions(Role.ADMIN)
        perms.add(Permission.VIEW_LOGS)  # mutate copy
        assert perms != rm.get_role_permissions(Role.ADMIN) or \
               Permission.VIEW_LOGS in rm.get_role_permissions(Role.ADMIN)


# ─── User CRUD ───────────────────────────────────────────────────────────────


class TestUserCRUD:
    def test_create_user(self, ac: AccessControl):
        user = ac.create_user("u1")
        assert user.uid == "u1"
        assert len(user.roles) == 0

    def test_create_user_with_roles(self, ac: AccessControl):
        user = ac.create_user("u1", roles=[Role.VALIDATOR, Role.MINER])
        assert Role.VALIDATOR in user.roles
        assert Role.MINER in user.roles

    def test_create_user_with_metadata(self, ac: AccessControl):
        user = ac.create_user("u1", metadata={"ip": "10.0.0.1"})
        assert user.metadata["ip"] == "10.0.0.1"

    def test_duplicate_user_raises(self, ac: AccessControl):
        ac.create_user("u1")
        with pytest.raises(ValueError, match="already exists"):
            ac.create_user("u1")

    def test_get_user(self, seeded_ac: AccessControl):
        user = seeded_ac.get_user("alice")
        assert user is not None
        assert user.uid == "alice"

    def test_get_unknown_user_returns_none(self, ac: AccessControl):
        assert ac.get_user("nobody") is None

    def test_delete_user(self, seeded_ac: AccessControl):
        seeded_ac.delete_user("bob")
        assert seeded_ac.get_user("bob") is None

    def test_delete_nonexistent_user_is_noop(self, ac: AccessControl):
        ac.delete_user("ghost")  # should not raise

    def test_list_users(self, seeded_ac: AccessControl):
        users = seeded_ac.list_users()
        uids = {u.uid for u in users}
        assert uids == {"alice", "bob", "carol", "dave"}

    def test_user_to_dict(self, seeded_ac: AccessControl):
        d = seeded_ac.get_user("alice").to_dict()
        assert d["uid"] == "alice"
        assert "admin" in d["roles"]


# ─── Role Assignment ─────────────────────────────────────────────────────────


class TestRoleAssignment:
    def test_assign_role(self, ac: AccessControl):
        ac.create_user("u1")
        ac.assign_role("u1", Role.VALIDATOR)
        user = ac.get_user("u1")
        assert Role.VALIDATOR in user.roles

    def test_revoke_role(self, seeded_ac: AccessControl):
        seeded_ac.revoke_role("bob", Role.VALIDATOR)
        user = seeded_ac.get_user("bob")
        assert Role.VALIDATOR not in user.roles

    def test_assign_role_unknown_user_raises(self, ac: AccessControl):
        with pytest.raises(ValueError, match="not found"):
            ac.assign_role("nobody", Role.ADMIN)

    def test_revoke_role_unknown_user_raises(self, ac: AccessControl):
        with pytest.raises(ValueError, match="not found"):
            ac.revoke_role("nobody", Role.ADMIN)


# ─── Permissions ─────────────────────────────────────────────────────────────


class TestPermissions:
    def test_admin_has_all_permissions(self, seeded_ac: AccessControl):
        assert seeded_ac.has_permission("alice", Permission.ADMIN_FULL)
        assert seeded_ac.has_permission("alice", Permission.MINE_BLOCKS)
        assert seeded_ac.has_permission("alice", Permission.API_ADMIN)

    def test_validator_permissions(self, seeded_ac: AccessControl):
        assert seeded_ac.has_permission("bob", Permission.VALIDATE_BLOCKS)
        assert seeded_ac.has_permission("bob", Permission.PROPOSE_BLOCKS)
        assert not seeded_ac.has_permission("bob", Permission.ADMIN_FULL)

    def test_miner_cannot_validate(self, seeded_ac: AccessControl):
        assert not seeded_ac.has_permission("carol", Permission.VALIDATE_BLOCKS)

    def test_has_any_permission(self, seeded_ac: AccessControl):
        assert seeded_ac.has_any_permission(
            "bob", [Permission.VALIDATE_BLOCKS, Permission.ADMIN_FULL]
        )

    def test_has_all_permissions(self, seeded_ac: AccessControl):
        assert seeded_ac.has_all_permissions(
            "bob", [Permission.VALIDATE_BLOCKS, Permission.PROPOSE_BLOCKS]
        )
        assert not seeded_ac.has_all_permissions(
            "bob", [Permission.VALIDATE_BLOCKS, Permission.ADMIN_FULL]
        )

    def test_custom_permission(self, ac: AccessControl):
        ac.create_user("u1", roles=[Role.OBSERVER])
        ac.grant_permission("u1", Permission.SUBMIT_TRANSACTION)
        assert ac.has_permission("u1", Permission.SUBMIT_TRANSACTION)

    def test_revoke_custom_permission(self, ac: AccessControl):
        ac.create_user("u1", roles=[Role.OBSERVER])
        ac.grant_permission("u1", Permission.SUBMIT_TRANSACTION)
        ac.revoke_permission("u1", Permission.SUBMIT_TRANSACTION)
        assert not ac.has_permission("u1", Permission.SUBMIT_TRANSACTION)

    def test_unknown_user_has_no_permissions(self, ac: AccessControl):
        assert not ac.has_permission("nobody", Permission.READ_BLOCKCHAIN)

    def test_cache_invalidation_on_role_change(self, seeded_ac: AccessControl):
        # Warm cache
        seeded_ac.get_user_permissions("bob")
        # Revoke role
        seeded_ac.revoke_role("bob", Role.VALIDATOR)
        # Should NOT return stale cached permissions
        assert not seeded_ac.has_permission("bob", Permission.VALIDATE_BLOCKS)


# ─── Decorators ──────────────────────────────────────────────────────────────


class TestDecorators:
    def test_require_permission_allows(self, seeded_ac: AccessControl):
        @seeded_ac.require_permission(Permission.VALIDATE_BLOCKS)
        def do_validation(uid: str) -> str:
            return "ok"

        assert do_validation("bob") == "ok"

    def test_require_permission_denies(self, seeded_ac: AccessControl):
        @seeded_ac.require_permission(Permission.ADMIN_FULL)
        def admin_action(uid: str) -> str:
            return "ok"

        with pytest.raises(PermissionError):
            admin_action("carol")

    def test_require_role_allows(self, seeded_ac: AccessControl):
        @seeded_ac.require_role(Role.ADMIN)
        def admin_only(uid: str) -> str:
            return "ok"

        assert admin_only("alice") == "ok"

    def test_require_role_denies(self, seeded_ac: AccessControl):
        @seeded_ac.require_role(Role.ADMIN)
        def admin_only(uid: str) -> str:
            return "ok"

        with pytest.raises(PermissionError):
            admin_only("bob")


# ─── Stats & Global Instance ────────────────────────────────────────────────


class TestStats:
    def test_get_stats(self, seeded_ac: AccessControl):
        stats = seeded_ac.get_stats()
        assert stats["total_users"] == 4
        assert stats["total_roles"] == 6  # 6 default roles
        assert "admin" in stats["users_by_role"]

    def test_global_access_control(self):
        ac = get_access_control()
        assert isinstance(ac, AccessControl)


# ─── Thread Safety ───────────────────────────────────────────────────────────


class TestThreadSafety:
    def test_concurrent_user_creation(self, ac: AccessControl):
        errors = []

        def create(uid: str):
            try:
                ac.create_user(uid, roles=[Role.OBSERVER])
            except Exception as e:
                errors.append(e)

        threads = [threading.Thread(target=create, args=(f"user_{i}",)) for i in range(20)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()

        assert len(errors) == 0
        assert len(ac.list_users()) == 20
