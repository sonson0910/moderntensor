"""
GraphQL API for ModernTensor

Provides GraphQL endpoints for querying the ModernTensor blockchain.
Built with Strawberry GraphQL for type-safe queries and mutations.
"""

import logging
from typing import Optional, List
import strawberry
from strawberry.fastapi import GraphQLRouter

from sdk.luxtensor_client import LuxtensorClient
from sdk.chain_data import NeuronInfo, SubnetInfo


logger = logging.getLogger(__name__)


@strawberry.type
class NeuronType:
    """GraphQL type for Neuron information."""
    uid: int
    hotkey: str
    coldkey: str
    active: bool
    subnet_uid: int
    stake: float
    total_stake: float
    rank: float
    trust: float
    consensus: float
    incentive: float
    dividends: float
    emission: float
    validator_permit: bool
    validator_trust: float
    last_update: int
    priority: float


@strawberry.type
class SubnetType:
    """GraphQL type for Subnet information."""
    subnet_uid: int
    netuid: int
    name: str
    owner: str
    n: int
    max_n: int
    emission_value: float
    tempo: int
    block: int
    burn: float


@strawberry.type
class BlockType:
    """GraphQL type for Block information."""
    block_number: int
    block_hash: str
    parent_hash: str
    timestamp: int
    num_transactions: int


@strawberry.type
class Query:
    """GraphQL Query root."""
    
    @strawberry.field
    def neuron(
        self,
        info,
        uid: int,
        subnet_uid: int
    ) -> Optional[NeuronType]:
        """Get neuron by UID and subnet UID."""
        client: LuxtensorClient = info.context["client"]
        
        try:
            neuron = client.get_neuron(uid, subnet_uid)
            if not neuron:
                return None
            
            return NeuronType(
                uid=neuron.uid,
                hotkey=neuron.hotkey,
                coldkey=neuron.coldkey,
                active=neuron.active,
                subnet_uid=neuron.subnet_uid,
                stake=neuron.stake,
                total_stake=neuron.total_stake,
                rank=neuron.rank,
                trust=neuron.trust,
                consensus=neuron.consensus,
                incentive=neuron.incentive,
                dividends=neuron.dividends,
                emission=neuron.emission,
                validator_permit=neuron.validator_permit,
                validator_trust=neuron.validator_trust,
                last_update=neuron.last_update,
                priority=neuron.priority,
            )
        except Exception as e:
            logger.error(f"Error fetching neuron: {e}")
            return None
    
    @strawberry.field
    def neurons(
        self,
        info,
        subnet_uid: int,
        limit: Optional[int] = None
    ) -> List[NeuronType]:
        """Get all neurons in a subnet."""
        client: LuxtensorClient = info.context["client"]
        
        try:
            neurons = client.get_neurons(subnet_uid)
            
            if limit:
                neurons = neurons[:limit]
            
            return [
                NeuronType(
                    uid=n.uid,
                    hotkey=n.hotkey,
                    coldkey=n.coldkey,
                    active=n.active,
                    subnet_uid=n.subnet_uid,
                    stake=n.stake,
                    total_stake=n.total_stake,
                    rank=n.rank,
                    trust=n.trust,
                    consensus=n.consensus,
                    incentive=n.incentive,
                    dividends=n.dividends,
                    emission=n.emission,
                    validator_permit=n.validator_permit,
                    validator_trust=n.validator_trust,
                    last_update=n.last_update,
                    priority=n.priority,
                )
                for n in neurons
            ]
        except Exception as e:
            logger.error(f"Error fetching neurons: {e}")
            return []
    
    @strawberry.field
    def subnet(
        self,
        info,
        subnet_uid: int
    ) -> Optional[SubnetType]:
        """Get subnet by UID."""
        client: LuxtensorClient = info.context["client"]
        
        try:
            subnet = client.get_subnet_info(subnet_uid)
            if not subnet:
                return None
            
            return SubnetType(
                subnet_uid=subnet.subnet_uid,
                netuid=subnet.netuid,
                name=subnet.name,
                owner=subnet.owner,
                n=subnet.n,
                max_n=subnet.max_n,
                emission_value=subnet.emission_value,
                tempo=subnet.tempo,
                block=subnet.block,
                burn=subnet.burn,
            )
        except Exception as e:
            logger.error(f"Error fetching subnet: {e}")
            return None
    
    @strawberry.field
    def subnets(
        self,
        info,
        limit: Optional[int] = None
    ) -> List[SubnetType]:
        """Get all subnets."""
        client: LuxtensorClient = info.context["client"]
        
        try:
            subnets = client.get_subnets()
            
            if limit:
                subnets = subnets[:limit]
            
            return [
                SubnetType(
                    subnet_uid=s.subnet_uid,
                    netuid=s.netuid,
                    name=s.name,
                    owner=s.owner,
                    n=s.n,
                    max_n=s.max_n,
                    emission_value=s.emission_value,
                    tempo=s.tempo,
                    block=s.block,
                    burn=s.burn,
                )
                for s in subnets
            ]
        except Exception as e:
            logger.error(f"Error fetching subnets: {e}")
            return []
    
    @strawberry.field
    def block_number(self, info) -> int:
        """Get current block number."""
        client: LuxtensorClient = info.context["client"]
        
        try:
            return client.get_block_number()
        except Exception as e:
            logger.error(f"Error fetching block number: {e}")
            return 0
    
    @strawberry.field
    def balance(
        self,
        info,
        address: str
    ) -> float:
        """Get account balance."""
        client: LuxtensorClient = info.context["client"]
        
        try:
            return client.get_balance(address)
        except Exception as e:
            logger.error(f"Error fetching balance: {e}")
            return 0.0


class GraphQLAPI:
    """
    GraphQL API server for ModernTensor network.
    
    Provides type-safe GraphQL queries for blockchain data.
    
    Example:
        ```python
        from sdk.api import GraphQLAPI
        from sdk.luxtensor_client import LuxtensorClient
        
        client = LuxtensorClient("http://localhost:9933")
        graphql_api = GraphQLAPI(client)
        
        # Add to FastAPI app
        app.include_router(graphql_api.router, prefix="/graphql")
        ```
    
    Example queries:
        ```graphql
        # Get neuron
        query {
          neuron(uid: 0, subnetUid: 1) {
            uid
            hotkey
            stake
            rank
          }
        }
        
        # Get all neurons
        query {
          neurons(subnetUid: 1, limit: 10) {
            uid
            hotkey
            stake
            validatorPermit
          }
        }
        
        # Get subnet
        query {
          subnet(subnetUid: 1) {
            name
            owner
            n
            maxN
          }
        }
        ```
    """
    
    def __init__(self, client: LuxtensorClient):
        """
        Initialize GraphQL API.
        
        Args:
            client: LuxtensorClient for blockchain queries
        """
        self.client = client
        
        # Create schema
        schema = strawberry.Schema(query=Query)
        
        # Create GraphQL router
        self.router = GraphQLRouter(
            schema,
            context_getter=self._get_context,
        )
        
        logger.info("Initialized GraphQL API")
    
    async def _get_context(self):
        """Provide context for GraphQL resolvers."""
        return {
            "client": self.client,
        }


__all__ = ["GraphQLAPI", "Query", "NeuronType", "SubnetType", "BlockType"]
