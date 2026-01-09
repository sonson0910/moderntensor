"""
ModernTensor SDK Version Information
"""

__version__ = "0.4.0"
__author__ = "ModernTensor Foundation"
__description__ = "Decentralized AI/ML training platform built on Luxtensor blockchain"
__url__ = "https://github.com/sonson0910/moderntensor"

# Version components
VERSION_MAJOR = 0
VERSION_MINOR = 4
VERSION_PATCH = 0

def get_version():
    """Get the current version string."""
    return __version__

def get_version_info():
    """Get detailed version information."""
    return {
        "version": __version__,
        "major": VERSION_MAJOR,
        "minor": VERSION_MINOR,
        "patch": VERSION_PATCH,
        "author": __author__,
        "description": __description__,
        "url": __url__,
    }

