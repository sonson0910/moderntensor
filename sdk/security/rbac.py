"""
Role-Based Access Control (RBAC) for ModernTensor.

This module provides RBAC capabilities for managing access control
across the ModernTensor network.
"""

import logging
import functools
import threading
from typing import Set, Dict, Optional, List
from enum import Enum
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone

logger = logging.getLogger(__name__)


class Permission(str, Enum):
    """System permissions."""

    # Blockchain permissions
    READ_BLOCKCHAIN = "blockchain.read"
    WRITE_BLOCKCHAIN = "blockchain.write"
    SUBMIT_TRANSACTION = "blockchain.submit_transaction"
    QUERY_STATE = "blockchain.query_state"

    # Network permissions
    READ_NETWORK = "network.read"
    WRITE_NETWORK = "network.write"
    MANAGE_PEERS = "network.manage_peers"
    BROADCAST_MESSAGE = "network.broadcast"

    # Validator permissions
    VALIDATE_BLOCKS = "validator.validate"
    PROPOSE_BLOCKS = "validator.propose"
    VOTE_CONSENSUS = "validator.vote"
    MANAGE_VALIDATORS = "validator.manage"

    # Miner permissions
    MINE_BLOCKS = "miner.mine"
    SUBMIT_WORK = "miner.submit_work"
    RECEIVE_REWARDS = "miner.rewards"

    # Admin permissions
    ADMIN_FULL = "admin.full"
    ADMIN_CONFIG = "admin.config"
    ADMIN_USERS = "admin.users"
    ADMIN_SECURITY = "admin.security"

    # API permissions
    API_READ = "api.read"
    API_WRITE = "api.write"
    API_ADMIN = "api.admin"

    # Monitoring permissions
    VIEW_METRICS = "monitoring.view"
    VIEW_LOGS = "monitoring.logs"
    MANAGE_ALERTS = "monitoring.alerts"


class Role(str, Enum):
    """System roles."""

    ADMIN = "admin"
    VALIDATOR = "validator"
    MINER = "miner"
    OBSERVER = "observer"
    API_USER = "api_user"
    DEVELOPER = "developer"


@dataclass
class User:
    """User representation."""

    uid: str
    roles: Set[Role] = field(default_factory=set)
    custom_permissions: Set[Permission] = field(default_factory=set)
    created_at: datetime = field(default_factory=lambda: datetime.now(timezone.utc))
    last_login: Optional[datetime] = None
    metadata: Dict[str, str] = field(default_factory=dict)

    def has_role(self, role: Role) -> bool:
        """Check if user has role."""
        return role in self.roles

    def add_role(self, role: Role):
        """Add role to user."""
        self.roles.add(role)

    def remove_role(self, role: Role):
        """Remove role from user."""
        self.roles.discard(role)

    def to_dict(self) -> Dict:
        """Convert user to dictionary."""
        return {
            "uid": self.uid,
            "roles": [r.value for r in self.roles],
            "custom_permissions": [p.value for p in self.custom_permissions],
            "created_at": self.created_at.isoformat(),
            "last_login": self.last_login.isoformat() if self.last_login else None,
            "metadata": self.metadata,
        }


class RoleManager:
    """
    Role manager for ModernTensor RBAC.

    Manages role definitions and their associated permissions.
    """

    def __init__(self):
        """Initialize role manager."""
        self._role_permissions: Dict[Role, Set[Permission]] = {}
        self._initialize_default_roles()

    def _initialize_default_roles(self):
        """Initialize default role permissions."""

        # Admin role - full access
        self._role_permissions[Role.ADMIN] = {
            Permission.ADMIN_FULL,
            Permission.ADMIN_CONFIG,
            Permission.ADMIN_USERS,
            Permission.ADMIN_SECURITY,
            Permission.READ_BLOCKCHAIN,
            Permission.WRITE_BLOCKCHAIN,
            Permission.SUBMIT_TRANSACTION,
            Permission.QUERY_STATE,
            Permission.READ_NETWORK,
            Permission.WRITE_NETWORK,
            Permission.MANAGE_PEERS,
            Permission.BROADCAST_MESSAGE,
            Permission.MANAGE_VALIDATORS,
            Permission.API_ADMIN,
            Permission.VIEW_METRICS,
            Permission.VIEW_LOGS,
            Permission.MANAGE_ALERTS,
        }

        # Validator role
        self._role_permissions[Role.VALIDATOR] = {
            Permission.READ_BLOCKCHAIN,
            Permission.SUBMIT_TRANSACTION,
            Permission.QUERY_STATE,
            Permission.READ_NETWORK,
            Permission.BROADCAST_MESSAGE,
            Permission.VALIDATE_BLOCKS,
            Permission.PROPOSE_BLOCKS,
            Permission.VOTE_CONSENSUS,
            Permission.RECEIVE_REWARDS,
            Permission.API_READ,
            Permission.API_WRITE,
            Permission.VIEW_METRICS,
        }

        # Miner role
        self._role_permissions[Role.MINER] = {
            Permission.READ_BLOCKCHAIN,
            Permission.SUBMIT_TRANSACTION,
            Permission.QUERY_STATE,
            Permission.READ_NETWORK,
            Permission.MINE_BLOCKS,
            Permission.SUBMIT_WORK,
            Permission.RECEIVE_REWARDS,
            Permission.API_READ,
            Permission.API_WRITE,
            Permission.VIEW_METRICS,
        }

        # Observer role - read-only
        self._role_permissions[Role.OBSERVER] = {
            Permission.READ_BLOCKCHAIN,
            Permission.QUERY_STATE,
            Permission.READ_NETWORK,
            Permission.API_READ,
            Permission.VIEW_METRICS,
            Permission.VIEW_LOGS,
        }

        # API user role
        self._role_permissions[Role.API_USER] = {
            Permission.READ_BLOCKCHAIN,
            Permission.QUERY_STATE,
            Permission.API_READ,
            Permission.API_WRITE,
        }

        # Developer role
        self._role_permissions[Role.DEVELOPER] = {
            Permission.READ_BLOCKCHAIN,
            Permission.SUBMIT_TRANSACTION,
            Permission.QUERY_STATE,
            Permission.READ_NETWORK,
            Permission.API_READ,
            Permission.API_WRITE,
            Permission.VIEW_METRICS,
            Permission.VIEW_LOGS,
        }

    def get_role_permissions(self, role: Role) -> Set[Permission]:
        """
        Get permissions for a role.

        Args:
            role: Role to get permissions for

        Returns:
            Set of permissions
        """
        return self._role_permissions.get(role, set()).copy()

    def add_permission_to_role(self, role: Role, permission: Permission):
        """
        Add permission to role.

        Args:
            role: Role to add permission to
            permission: Permission to add
        """
        if role not in self._role_permissions:
            self._role_permissions[role] = set()

        self._role_permissions[role].add(permission)
        logger.info(f"Added permission {permission.value} to role {role.value}")

    def remove_permission_from_role(self, role: Role, permission: Permission):
        """
        Remove permission from role.

        Args:
            role: Role to remove permission from
            permission: Permission to remove
        """
        if role in self._role_permissions:
            self._role_permissions[role].discard(permission)
            logger.info(f"Removed permission {permission.value} from role {role.value}")

    def list_roles(self) -> List[Role]:
        """List all defined roles."""
        return list(self._role_permissions.keys())


class AccessControl:
    """
    Access control manager for ModernTensor RBAC.

    Manages users, roles, and permission checks.
    """

    def __init__(self):
        """Initialize access control with thread-safe locking."""
        self._lock = threading.RLock()
        self.role_manager = RoleManager()
        self.users: Dict[str, User] = {}
        self._permission_cache: Dict[str, Set[Permission]] = {}
        self._cache_ttl = timedelta(minutes=5)
        self._cache_timestamps: Dict[str, datetime] = {}

    def create_user(
        self,
        uid: str,
        roles: Optional[List[Role]] = None,
        metadata: Optional[Dict[str, str]] = None,
    ) -> User:
        """
        Create a new user.

        Args:
            uid: User identifier
            roles: Initial roles
            metadata: User metadata

        Returns:
            Created user
        """
        with self._lock:
            if uid in self.users:
                raise ValueError(f"User {uid} already exists")

            user = User(
                uid=uid,
                roles=set(roles or []),
                metadata=metadata or {},
            )

            self.users[uid] = user
            logger.info(f"Created user {uid} with roles: {[r.value for r in user.roles]}")

            return user

    def get_user(self, uid: str) -> Optional[User]:
        """
        Get user by ID.

        Args:
            uid: User identifier

        Returns:
            User or None if not found
        """
        return self.users.get(uid)

    def delete_user(self, uid: str):
        """
        Delete user.

        Args:
            uid: User identifier
        """
        with self._lock:
            if uid in self.users:
                del self.users[uid]
                self._invalidate_cache(uid)
                logger.info(f"Deleted user {uid}")

    def assign_role(self, uid: str, role: Role):
        """
        Assign role to user.

        Args:
            uid: User identifier
            role: Role to assign
        """
        with self._lock:
            user = self.get_user(uid)
            if not user:
                raise ValueError(f"User {uid} not found")

            user.add_role(role)
            self._invalidate_cache(uid)
            logger.info(f"Assigned role {role.value} to user {uid}")

    def revoke_role(self, uid: str, role: Role):
        """
        Revoke role from user.

        Args:
            uid: User identifier
            role: Role to revoke
        """
        with self._lock:
            user = self.get_user(uid)
            if not user:
                raise ValueError(f"User {uid} not found")

            user.remove_role(role)
            self._invalidate_cache(uid)
            logger.info(f"Revoked role {role.value} from user {uid}")

    def grant_permission(self, uid: str, permission: Permission):
        """
        Grant custom permission to user.

        Args:
            uid: User identifier
            permission: Permission to grant
        """
        with self._lock:
            user = self.get_user(uid)
            if not user:
                raise ValueError(f"User {uid} not found")

            user.custom_permissions.add(permission)
            self._invalidate_cache(uid)
            logger.info(f"Granted permission {permission.value} to user {uid}")

    def revoke_permission(self, uid: str, permission: Permission):
        """
        Revoke custom permission from user.

        Args:
            uid: User identifier
            permission: Permission to revoke
        """
        with self._lock:
            user = self.get_user(uid)
            if not user:
                raise ValueError(f"User {uid} not found")

            user.custom_permissions.discard(permission)
            self._invalidate_cache(uid)
            logger.info(f"Revoked permission {permission.value} from user {uid}")

    def get_user_permissions(self, uid: str) -> Set[Permission]:
        """
        Get all permissions for a user.

        Args:
            uid: User identifier

        Returns:
            Set of permissions
        """
        with self._lock:
            # Check cache
            if uid in self._permission_cache:
                cache_time = self._cache_timestamps.get(uid)
                if cache_time and datetime.now(timezone.utc) - cache_time < self._cache_ttl:
                    return self._permission_cache[uid].copy()

            user = self.get_user(uid)
            if not user:
                return set()

            # Aggregate permissions from roles
            permissions = set()
            for role in user.roles:
                permissions.update(self.role_manager.get_role_permissions(role))

            # Add custom permissions
            permissions.update(user.custom_permissions)

            # Cache result
            self._permission_cache[uid] = permissions
            self._cache_timestamps[uid] = datetime.now(timezone.utc)

            return permissions.copy()

    def has_permission(self, uid: str, permission: Permission) -> bool:
        """
        Check if user has permission.

        Args:
            uid: User identifier
            permission: Permission to check

        Returns:
            True if user has permission, False otherwise
        """
        # Admin always has permission
        user = self.get_user(uid)
        if user and Role.ADMIN in user.roles:
            return True

        permissions = self.get_user_permissions(uid)
        return permission in permissions

    def has_any_permission(self, uid: str, permissions: List[Permission]) -> bool:
        """
        Check if user has any of the given permissions.

        Args:
            uid: User identifier
            permissions: List of permissions to check

        Returns:
            True if user has any permission, False otherwise
        """
        user_permissions = self.get_user_permissions(uid)
        return any(p in user_permissions for p in permissions)

    def has_all_permissions(self, uid: str, permissions: List[Permission]) -> bool:
        """
        Check if user has all given permissions.

        Args:
            uid: User identifier
            permissions: List of permissions to check

        Returns:
            True if user has all permissions, False otherwise
        """
        user_permissions = self.get_user_permissions(uid)
        return all(p in user_permissions for p in permissions)

    def _invalidate_cache(self, uid: str):
        """Invalidate permission cache for user."""
        self._permission_cache.pop(uid, None)
        self._cache_timestamps.pop(uid, None)

    def require_permission(self, permission: Permission):
        """
        Decorator to require permission for a function.

        Args:
            permission: Required permission

        Returns:
            Decorated function
        """
        def decorator(func):
            @functools.wraps(func)
            def wrapper(uid: str, *args, **kwargs):
                if not self.has_permission(uid, permission):
                    raise PermissionError(
                        f"User {uid} does not have permission {permission.value}"
                    )
                return func(uid, *args, **kwargs)
            return wrapper
        return decorator

    def require_role(self, role: Role):
        """
        Decorator to require role for a function.

        Args:
            role: Required role

        Returns:
            Decorated function
        """
        def decorator(func):
            @functools.wraps(func)
            def wrapper(uid: str, *args, **kwargs):
                user = self.get_user(uid)
                if not user or not user.has_role(role):
                    raise PermissionError(
                        f"User {uid} does not have role {role.value}"
                    )
                return func(uid, *args, **kwargs)
            return wrapper
        return decorator

    def list_users(self) -> List[User]:
        """List all users."""
        return list(self.users.values())

    def get_stats(self) -> Dict:
        """Get RBAC statistics."""
        role_counts = {}
        for user in self.users.values():
            for role in user.roles:
                role_counts[role.value] = role_counts.get(role.value, 0) + 1

        return {
            "total_users": len(self.users),
            "total_roles": len(self.role_manager.list_roles()),
            "users_by_role": role_counts,
            "cache_size": len(self._permission_cache),
        }


# Global access control instance
global_access_control = AccessControl()


def get_access_control() -> AccessControl:
    """
    Get the global access control instance.

    Returns:
        Global AccessControl instance
    """
    return global_access_control
