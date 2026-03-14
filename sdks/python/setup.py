"""
UPP Python SDK setup script
"""

from setuptools import setup, find_packages

with open('README.md', 'r', encoding='utf-8') as f:
    long_description = f.read()

setup(
    name='upp-sdk',
    version='0.1.0',
    description='Universal Prediction Protocol (UPP) Python SDK',
    long_description=long_description,
    long_description_content_type='text/markdown',
    author='UPP Team',
    author_email='team@upp.dev',
    url='https://github.com/universal-prediction-protocol/upp',
    license='Apache-2.0',
    packages=find_packages(),
    python_requires='>=3.8',
    install_requires=[
        'httpx>=0.24.0',
        'requests>=2.28.0',
    ],
    extras_require={
        'dev': [
            'pytest>=7.0.0',
            'pytest-asyncio>=0.20.0',
            'black>=22.0.0',
            'isort>=5.0.0',
            'mypy>=0.990',
            'flake8>=4.0.0',
        ],
    },
    classifiers=[
        'Development Status :: 3 - Alpha',
        'Intended Audience :: Developers',
        'License :: OSI Approved :: Apache Software License',
        'Programming Language :: Python :: 3',
        'Programming Language :: Python :: 3.8',
        'Programming Language :: Python :: 3.9',
        'Programming Language :: Python :: 3.10',
        'Programming Language :: Python :: 3.11',
        'Programming Language :: Python :: 3.12',
        'Topic :: Software Development :: Libraries :: Python Modules',
        'Topic :: Office/Business :: Financial',
    ],
    keywords='upp prediction-markets arbitrage trading api',
)
