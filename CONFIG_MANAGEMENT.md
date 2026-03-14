# UPP Gateway Configuration Management Guide

This document describes the enhanced configuration management system for UPP Gateway, which supports environment-specific configurations, flexible overrides, and secure secret management.

## Quick Start

1. Copy the example environment file:
   ```bash
   cp .env.example .env
   ```

2. Set your environment:
   ```bash
   export UPP_ENVIRONMENT=dev  # or staging, prod
   ```

3. Run the gateway:
   ```bash
   cargo run --bin gateway
   ```

## Configuration Hierarchy

The configuration system follows a well-defined precedence order (highest to lowest):

1. **Environment Variables** (UPP_*)
   - Override all file-based configurations
   - Highest priority for flexibility in deployments

2. **Environment-Specific TOML Files** (config/gateway.{env}.toml)
   - Applied after base configuration
   - Environment names: dev, staging, prod
   - Only loaded if they exist and match UPP_ENVIRONMENT

3. **Base TOML File** (config/gateway.toml)
   - Default settings for all environments
   - Foundation that env-specific files override

4. **Hardcoded Defaults** (rust code defaults)
   - Lowest priority
   - Fallback values defined in config.rs

### Example Load Sequence for Staging

When `UPP_ENVIRONMENT=staging`:

```
Defaults (rust code)
    ↓
config/gateway.toml (base overrides)
    ↓
config/gateway.staging.toml (staging overrides)
    ↓
UPP_* Environment variables (final overrides)
```

## Configuration Files

### 1. Base Configuration: config/gateway.toml

Default settings shared across all environments:

```toml
environment = "dev"
host = "0.0.0.0"
port = 8080
grpc_port = 50051
log_format = "json"
auth_required = false
max_connections = 10000
graceful_shutdown_timeout_secs = 30
```

**When to edit:**
- Update core defaults used by all environments
- Add new configuration parameters
- Change cache TTLs that apply everywhere

### 2. Development: config/gateway.dev.toml

Overrides for local development with relaxed settings:

```toml
environment = "dev"
log_format = "pretty"           # Human-readable logs
auth_required = false           # No authentication
rate_limit_light_rps = 1000.0   # Very relaxed
rate_limit_ws_rps = 25.0
graceful_shutdown_timeout_secs = 10
```

**Use when:** Working locally or in development environments

### 3. Staging: config/gateway.staging.toml

Production-like settings with moderate restrictions:

```toml
environment = "staging"
log_format = "json"
auth_required = false           # Can be toggled
rate_limit_standard_rps = 50.0  # Moderate
max_connections = 5000
cors_origins = [
    "https://staging.example.com",
    "https://app-staging.example.com",
]
```

**Use when:** Testing in staging before production deployment

### 4. Production: config/gateway.prod.toml

Strict settings for production:

```toml
environment = "prod"
log_format = "json"             # Structured logging
auth_required = true            # Authentication required
rate_limit_standard_rps = 20.0  # Strict limits
max_connections = 5000
graceful_shutdown_timeout_secs = 60
cors_origins = ["https://example.com"]
```

**Use when:** Running in production

## Environment Variables

All configuration fields can be overridden via environment variables with the `UPP_` prefix.

### Core Variables

```bash
# Environment selection
UPP_ENVIRONMENT=dev|staging|prod

# Server
UPP_HOST=0.0.0.0
UPP_PORT=8080
UPP_GRPC_PORT=50051

# Logging
UPP_LOG_FORMAT=json|pretty
```

### Authentication

```bash
UPP_AUTH_REQUIRED=true|false
UPP_JWT_SECRET=your-secret-key
UPP_CORS_ORIGINS=https://example.com,https://app.example.com
```

### Rate Limiting (per tier)

```bash
# Light tier
UPP_RATE_LIMIT_LIGHT_BURST=200
UPP_RATE_LIMIT_LIGHT_RPS=100.0

# Standard tier
UPP_RATE_LIMIT_STANDARD_BURST=50
UPP_RATE_LIMIT_STANDARD_RPS=20.0

# Heavy tier
UPP_RATE_LIMIT_HEAVY_BURST=20
UPP_RATE_LIMIT_HEAVY_RPS=5.0

# WebSocket tier
UPP_RATE_LIMIT_WS_BURST=10
UPP_RATE_LIMIT_WS_RPS=2.0
```

### Provider Credentials

```bash
UPP_KALSHI_API_KEY_ID=key-id
UPP_KALSHI_PRIVATE_KEY_PATH=/path/to/key
UPP_POLYMARKET_WALLET_KEY=0x...
UPP_OPINION_API_KEY=api-key
```

### Cache & Storage

```bash
UPP_MARKET_CACHE_TTL_SECONDS=300
UPP_ORDERBOOK_CACHE_TTL_MS=500
UPP_REDIS_URL=redis://localhost:6379
```

### Connection Management

```bash
UPP_MAX_CONNECTIONS=10000
UPP_GRACEFUL_SHUTDOWN_TIMEOUT_SECS=30
```

### TLS/SSL

```bash
UPP_TLS_CERT_PATH=/path/to/cert.pem
UPP_TLS_KEY_PATH=/path/to/key.pem
```

## Secrets Management

### For Development

1. Create `.env` from `.env.example`:
   ```bash
   cp .env.example .env
   ```

2. Fill in actual values:
   ```bash
   UPP_KALSHI_API_KEY_ID=your-actual-key
   UPP_JWT_SECRET=your-actual-secret
   ```

3. Ensure `.env` is in `.gitignore` (already configured)

### For Staging & Production

**Never commit secrets to version control!**

Instead, use one of these approaches:

#### Option 1: Environment Variables (Recommended)

Set secrets directly as environment variables:

```bash
export UPP_JWT_SECRET="actual-secret-key"
export UPP_KALSHI_API_KEY_ID="actual-api-key"
# ... other secrets
```

#### Option 2: Secrets Management Tools

Use cloud provider secret stores:

- **AWS**: Use AWS Secrets Manager or Parameter Store
- **Google Cloud**: Use Google Cloud Secret Manager
- **Azure**: Use Azure Key Vault
- **Kubernetes**: Use K8s Secrets
- **HashiCorp**: Use Vault

Example with AWS Secrets Manager:

```bash
# Fetch secrets and set as environment variables
export UPP_JWT_SECRET=$(aws secretsmanager get-secret-value --secret-id upp/jwt-secret --query SecretString --output text)
export UPP_KALSHI_API_KEY_ID=$(aws secretsmanager get-secret-value --secret-id upp/kalshi-key --query SecretString --output text)

# Run gateway
cargo run --bin gateway
```

#### Option 3: Encrypted Config Files

If using encrypted config files:

1. Create `config/secrets.toml` with actual values
2. Encrypt with a tool like `sops` or `git-crypt`:
   ```bash
   sops config/secrets.toml  # Edit encrypted file
   ```
3. Add to source control (encrypted)
4. Decrypt at runtime in deployment scripts

## Configuration API

The `GatewayConfig` struct provides helper methods:

```rust
use upp_gateway::core::config::GatewayConfig;

// Load configuration
let config = GatewayConfig::load()?;

// Check environment
if config.is_dev() {
    println!("Development mode");
}

if config.is_prod() {
    println!("Production mode");
}

// Get environment name
println!("Running in: {}", config.env_name());

// Or load specific environment
let staging_config = GatewayConfig::load_for_env("staging")?;
```

## Common Configuration Patterns

### Development Setup

```bash
# .env
UPP_ENVIRONMENT=dev
UPP_LOG_FORMAT=pretty
UPP_AUTH_REQUIRED=false
UPP_KALSHI_API_KEY_ID=test-key-123
```

Then run:
```bash
cargo run --bin gateway
```

### Docker with Environment Variables

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin gateway

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/gateway /usr/local/bin/
COPY config/ /etc/upp/config/
EXPOSE 8080
CMD ["gateway"]
```

Run with:
```bash
docker run \
  -e UPP_ENVIRONMENT=prod \
  -e UPP_JWT_SECRET="$SECRET_KEY" \
  -e UPP_KALSHI_API_KEY_ID="$API_KEY" \
  -e UPP_TLS_CERT_PATH=/etc/upp/certs/tls.crt \
  -e UPP_TLS_KEY_PATH=/etc/upp/certs/tls.key \
  -v /etc/upp/certs:/etc/upp/certs:ro \
  upp-gateway:latest
```

### Kubernetes ConfigMap + Secrets

```yaml
# ConfigMap for non-sensitive config
apiVersion: v1
kind: ConfigMap
metadata:
  name: upp-config
data:
  UPP_ENVIRONMENT: "prod"
  UPP_LOG_FORMAT: "json"
  UPP_PORT: "8080"

---
# Secret for sensitive values
apiVersion: v1
kind: Secret
metadata:
  name: upp-secrets
type: Opaque
stringData:
  UPP_JWT_SECRET: "actual-secret-key"
  UPP_KALSHI_API_KEY_ID: "actual-api-key"
  UPP_TLS_CERT: |
    -----BEGIN CERTIFICATE-----
    ...
    -----END CERTIFICATE-----
  UPP_TLS_KEY: |
    -----BEGIN PRIVATE KEY-----
    ...
    -----END PRIVATE KEY-----

---
apiVersion: v1
kind: Pod
metadata:
  name: upp-gateway
spec:
  containers:
  - name: gateway
    image: upp-gateway:latest
    envFrom:
    - configMapRef:
        name: upp-config
    - secretRef:
        name: upp-secrets
    ports:
    - containerPort: 8080
```

## Environment-Specific Recommendations

### Development

- Use `config/gateway.dev.toml` with relaxed limits
- Set `log_format = "pretty"` for readability
- Disable authentication with `auth_required = false`
- Use `.env` for local credentials

### Staging

- Use `config/gateway.staging.toml`
- Enable logging with `log_format = "json"`
- Mirror production limits (but slightly relaxed)
- Use environment variables for secrets
- Test with real provider APIs

### Production

- Use `config/gateway.prod.toml`
- Always enable authentication: `auth_required = true`
- Use `log_format = "json"` for log aggregation
- Strict rate limits to prevent abuse
- Set secrets via environment variables or secret manager
- Enable TLS with valid certificates
- Use managed Redis with encryption
- Monitor and alert on configuration changes

## Validation

Configuration is validated at startup via `ConfigValidator::validate_all()`:

```bash
$ UPP_ENVIRONMENT=prod cargo run --bin gateway
# Error: Configuration validation failed
# - auth_required=true but jwt_secret not set
# - tls_cert_path required for production
# - redis_url recommended for production
```

The validator checks:
- Required fields for the environment
- Sensible value ranges
- TLS certificate validity
- Provider credentials availability

## Troubleshooting

### Configuration not loading

1. Check file permissions:
   ```bash
   ls -la config/gateway*.toml
   ```

2. Verify file format:
   ```bash
   cargo run --bin gateway 2>&1 | grep -i config
   ```

3. Check environment variable:
   ```bash
   echo $UPP_ENVIRONMENT
   ```

### Environment variable not overriding config

Ensure it uses correct naming: `UPP_` prefix + snake_case field names:

```bash
# WRONG
UPP_grpc-port=50051

# CORRECT
UPP_GRPC_PORT=50051
```

### Secrets not being loaded

1. For `.env` files: ensure file exists and is readable
2. For env vars: verify they're exported: `export VAR=value`
3. For secrets managers: verify authentication and permissions

### Rate limits too strict/loose

Adjust in TOML files or via env vars:

```bash
# Override in prod
export UPP_RATE_LIMIT_STANDARD_RPS=50.0
```

## Implementation Details

### Config Loading Code

The configuration is loaded in `gateway/src/core/config.rs`:

1. Load base configuration (`config/gateway.toml`)
2. Overlay environment-specific file (`config/gateway.{env}.toml`)
3. Apply environment variables (`UPP_*`)
4. Deserialize to `GatewayConfig` struct

### Default Values

All fields have sensible defaults defined in the rust code, ensuring the gateway works even with minimal configuration.

### Type Safety

Configuration is strongly typed through Rust's serde/deserialize system:
- Invalid values at startup → clear error messages
- Type mismatches caught before runtime
- JSON/TOML parsing errors logged

## Security Considerations

1. **File Permissions**
   - Config files: 644 (readable by all, writable by owner)
   - Secrets files: 600 (readable only by owner)
   - Private key files: 400 (owner read-only)

2. **Secret Rotation**
   - API keys: Rotate quarterly or when employees leave
   - JWT secrets: Can rotate transparently
   - TLS certs: Renew before expiration

3. **Audit Logging**
   - Log configuration loads (for compliance)
   - Monitor secret access
   - Alert on unexpected configuration changes

4. **Version Control**
   - Never commit `.env` or `config/secrets.toml`
   - Commit `.env.example` and `config/secrets.example.toml`
   - Use `.gitignore` to prevent accidental commits

## References

- Config crate: https://github.com/mehcode/config-rs
- dotenvy crate: https://github.com/allan2/dotenvy
- Environment variables: https://12factor.net/config
- Secrets management best practices: https://owasp.org/www-project-secret-management
