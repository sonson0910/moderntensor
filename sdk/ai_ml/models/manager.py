"""
Advanced Model Manager - Surpassing Bittensor's capabilities.

Features that Bittensor doesn't have:
- Model versioning and experiment tracking
- Automatic model loading and caching
- Model performance benchmarking
- Model registry with metadata
- Distributed model serving support
"""

import hashlib
import logging
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional, Callable
from enum import Enum

logger = logging.getLogger(__name__)


class ModelStatus(Enum):
    """Status of a model"""
    LOADING = "loading"
    READY = "ready"
    FAILED = "failed"
    DEPRECATED = "deprecated"


@dataclass
class ModelVersion:
    """Model version metadata"""
    version: str
    model_id: str
    created_at: float = field(default_factory=time.time)
    checksum: Optional[str] = None
    size_bytes: int = 0
    metadata: Dict[str, Any] = field(default_factory=dict)
    
    def __hash__(self):
        return hash((self.model_id, self.version))


@dataclass
class ModelMetadata:
    """Metadata for a model"""
    model_id: str
    name: str
    description: str
    framework: str  # "pytorch", "tensorflow", "huggingface", etc.
    task_type: str  # "text_generation", "image_classification", etc.
    versions: List[ModelVersion] = field(default_factory=list)
    latest_version: Optional[str] = None
    performance_metrics: Dict[str, Any] = field(default_factory=dict)
    tags: List[str] = field(default_factory=list)
    
    def add_version(self, version: ModelVersion) -> None:
        """Add a new version"""
        self.versions.append(version)
        self.latest_version = version.version
    
    def get_version(self, version: str) -> Optional[ModelVersion]:
        """Get specific version"""
        for v in self.versions:
            if v.version == version:
                return v
        return None


class ModelManager:
    """
    Advanced model manager that surpasses Bittensor.
    
    Features:
    - Model versioning with automatic tracking
    - Model registry with rich metadata
    - Lazy loading with caching
    - Performance benchmarking
    - Model health monitoring
    
    Example:
        manager = ModelManager()
        
        # Register model
        manager.register_model(
            model_id="gpt2-medium",
            name="GPT-2 Medium",
            framework="huggingface",
            task_type="text_generation",
        )
        
        # Load model
        model = manager.load_model("gpt2-medium", version="1.0.0")
        
        # Track performance
        manager.track_inference(model_id="gpt2-medium", latency_ms=150)
    """
    
    def __init__(self, cache_dir: Optional[Path] = None):
        """
        Initialize model manager.
        
        Args:
            cache_dir: Directory for caching models
        """
        self.cache_dir = cache_dir or Path.home() / ".moderntensor" / "models"
        self.cache_dir.mkdir(parents=True, exist_ok=True)
        
        # Model registry
        self.models: Dict[str, ModelMetadata] = {}
        
        # Loaded models cache
        self.loaded_models: Dict[str, Any] = {}
        
        # Model loaders (custom loading functions)
        self.loaders: Dict[str, Callable] = {}
        
        # Performance metrics
        self.inference_metrics: Dict[str, List[float]] = {}
        
        logger.info(f"ModelManager initialized with cache_dir={self.cache_dir}")
    
    def register_model(
        self,
        model_id: str,
        name: str,
        description: str,
        framework: str,
        task_type: str,
        tags: Optional[List[str]] = None,
    ) -> ModelMetadata:
        """
        Register a new model.
        
        Args:
            model_id: Unique identifier for the model
            name: Human-readable name
            description: Model description
            framework: Framework name ("pytorch", "tensorflow", etc.)
            task_type: Type of task ("text_generation", etc.)
            tags: Optional tags for categorization
        
        Returns:
            ModelMetadata object
        """
        if model_id in self.models:
            logger.warning(f"Model {model_id} already registered, updating...")
        
        metadata = ModelMetadata(
            model_id=model_id,
            name=name,
            description=description,
            framework=framework,
            task_type=task_type,
            tags=tags or [],
        )
        
        self.models[model_id] = metadata
        logger.info(f"Registered model: {model_id} ({name})")
        
        return metadata
    
    def add_version(
        self,
        model_id: str,
        version: str,
        path: Optional[Path] = None,
        metadata: Optional[Dict[str, Any]] = None,
    ) -> ModelVersion:
        """
        Add a new version for a model.
        
        Args:
            model_id: Model identifier
            version: Version string (e.g., "1.0.0")
            path: Path to model files
            metadata: Additional metadata
        
        Returns:
            ModelVersion object
        """
        if model_id not in self.models:
            raise ValueError(f"Model {model_id} not registered")
        
        # Calculate checksum if path provided
        checksum = None
        size_bytes = 0
        if path and path.exists():
            checksum = self._calculate_checksum(path)
            size_bytes = self._get_size(path)
        
        model_version = ModelVersion(
            version=version,
            model_id=model_id,
            checksum=checksum,
            size_bytes=size_bytes,
            metadata=metadata or {},
        )
        
        self.models[model_id].add_version(model_version)
        logger.info(f"Added version {version} for model {model_id}")
        
        return model_version
    
    def register_loader(
        self,
        model_id: str,
        loader_func: Callable,
    ) -> None:
        """
        Register a custom loader function for a model.
        
        Args:
            model_id: Model identifier
            loader_func: Function that loads the model
                        Signature: (model_id: str, version: str, **kwargs) -> Any
        """
        self.loaders[model_id] = loader_func
        logger.info(f"Registered custom loader for {model_id}")
    
    def load_model(
        self,
        model_id: str,
        version: Optional[str] = None,
        **kwargs
    ) -> Any:
        """
        Load a model (with caching).
        
        Args:
            model_id: Model identifier
            version: Specific version (uses latest if None)
            **kwargs: Additional arguments for loader
        
        Returns:
            Loaded model object
        """
        if model_id not in self.models:
            raise ValueError(f"Model {model_id} not registered")
        
        # Use latest version if not specified
        if version is None:
            version = self.models[model_id].latest_version
            if version is None:
                raise ValueError(f"No versions available for model {model_id}")
        
        # Check cache
        cache_key = f"{model_id}:{version}"
        if cache_key in self.loaded_models:
            logger.debug(f"Model {cache_key} loaded from cache")
            return self.loaded_models[cache_key]
        
        # Load model
        logger.info(f"Loading model {cache_key}...")
        start_time = time.time()
        
        if model_id in self.loaders:
            model = self.loaders[model_id](model_id, version, **kwargs)
        else:
            raise ValueError(f"No loader registered for model {model_id}")
        
        load_time = time.time() - start_time
        logger.info(f"Model {cache_key} loaded in {load_time:.2f}s")
        
        # Cache model
        self.loaded_models[cache_key] = model
        
        return model
    
    def unload_model(self, model_id: str, version: Optional[str] = None) -> None:
        """
        Unload a model from cache.
        
        Args:
            model_id: Model identifier
            version: Specific version (unloads all if None)
        """
        if version is None:
            # Unload all versions
            keys_to_remove = [k for k in self.loaded_models.keys() if k.startswith(f"{model_id}:")]
            for key in keys_to_remove:
                del self.loaded_models[key]
            logger.info(f"Unloaded all versions of {model_id}")
        else:
            cache_key = f"{model_id}:{version}"
            if cache_key in self.loaded_models:
                del self.loaded_models[cache_key]
                logger.info(f"Unloaded {cache_key}")
    
    def track_inference(
        self,
        model_id: str,
        latency_ms: float,
        batch_size: int = 1,
    ) -> None:
        """
        Track inference performance.
        
        Args:
            model_id: Model identifier
            latency_ms: Inference latency in milliseconds
            batch_size: Batch size used
        """
        if model_id not in self.inference_metrics:
            self.inference_metrics[model_id] = []
        
        self.inference_metrics[model_id].append(latency_ms)
        
        # Update model metadata
        if model_id in self.models:
            metrics = self.inference_metrics[model_id]
            self.models[model_id].performance_metrics = {
                "avg_latency_ms": sum(metrics) / len(metrics),
                "min_latency_ms": min(metrics),
                "max_latency_ms": max(metrics),
                "total_inferences": len(metrics),
            }
    
    def get_model_metadata(self, model_id: str) -> Optional[ModelMetadata]:
        """Get metadata for a model"""
        return self.models.get(model_id)
    
    def list_models(
        self,
        framework: Optional[str] = None,
        task_type: Optional[str] = None,
        tags: Optional[List[str]] = None,
    ) -> List[ModelMetadata]:
        """
        List registered models with optional filtering.
        
        Args:
            framework: Filter by framework
            task_type: Filter by task type
            tags: Filter by tags (must have all tags)
        
        Returns:
            List of ModelMetadata objects
        """
        models = list(self.models.values())
        
        if framework:
            models = [m for m in models if m.framework == framework]
        
        if task_type:
            models = [m for m in models if m.task_type == task_type]
        
        if tags:
            models = [m for m in models if all(t in m.tags for t in tags)]
        
        return models
    
    def get_performance_summary(self) -> Dict[str, Any]:
        """Get performance summary for all models"""
        summary = {}
        for model_id, metadata in self.models.items():
            summary[model_id] = {
                "name": metadata.name,
                "performance": metadata.performance_metrics,
                "versions": len(metadata.versions),
                "latest_version": metadata.latest_version,
            }
        return summary
    
    def _calculate_checksum(self, path: Path) -> str:
        """Calculate SHA256 checksum of file or directory"""
        sha256 = hashlib.sha256()
        
        if path.is_file():
            with open(path, "rb") as f:
                for chunk in iter(lambda: f.read(8192), b""):
                    sha256.update(chunk)
        else:
            # For directories, hash all files
            for file_path in sorted(path.rglob("*")):
                if file_path.is_file():
                    with open(file_path, "rb") as f:
                        for chunk in iter(lambda: f.read(8192), b""):
                            sha256.update(chunk)
        
        return sha256.hexdigest()
    
    def _get_size(self, path: Path) -> int:
        """Get total size in bytes"""
        if path.is_file():
            return path.stat().st_size
        else:
            return sum(f.stat().st_size for f in path.rglob("*") if f.is_file())
