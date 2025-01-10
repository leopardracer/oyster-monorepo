![Marlin Oyster Logo](./logo.svg)

# Oyster CVM CLI

A command line utility to manage Oyster CVM lifecycle: build, upload, deploy and verify. Get started easily with just a `docker-compose` file.

## Prerequisites

- Docker
- Nix
- Git (for fetching flakes)

## Installation

From source:

```bash
git clone https://github.com/marlinprotocol/oyster-monorepo.git
cd cli/oyster-cvm
cargo build --release
```

## Usage

### View help

```bash
oyster-cvm --help
```

### Commands

#### `doctor`

Checks if Docker and Nix are installed.

#### `build`
Builds an oyster-cvm image.

Required args:
- `--platform` (amd64 or arm64)
- `--docker-compose` (path to docker-compose.yml)

Optional args:
- `--docker-images` (list of Docker .tar files to be loaded)
- `--output` (output directory, default: result)

#### `upload`
Uploads an enclave image to IPFS via Pinata.

Required args:
- `--file` (path to the enclave image file)

Required env vars:
["PINATA_API_KEY", "PINATA_API_SECRET"]

#### `verify`
Verifies an Oyster enclave's attestation document.

Required args:
- `--enclave-ip` (-e): Enclave IP address

Optional args:
- `--pcr0` (-0): PCR0 value
- `--pcr1` (-1): PCR1 value
- `--pcr2` (-2): PCR2 value
- `--attestation-port` (-p): Attestation port (default: 1300)
- `--max-age` (-a): Maximum age of attestation in milliseconds (default: 300000)
- `--timestamp` (-t): Attestation timestamp in milliseconds (default: 0)
- `--root-public-key` (-r): Root public key (defaults to AWS root key)

#### `deploy`
Deploys an Oyster CVM instance.

Required args:
- `--image-url`: URL of the enclave image
- `--region`: Region for deployment
- `--wallet-private-key`: Wallet private key for transaction signing
- `--operator`: Operator address
- `--instance-type`: Instance type (e.g. "m5a.2xlarge")
- `--duration-in-minutes`: Duration in minutes

Optional args:
- `--bandwidth`: Bandwidth in KBps (default: 10)
- `--job-name`: Job name
- `--debug`: Start enclave in debug mode

#### `logs`
Streams logs from an Oyster CVM instance.

Required args:
- `--ip` (-i): IP address of the instance (required)

Optional args:
- `--start-from` (-s): Optional log ID to start streaming from
- `--with-log-id`(-w): Include log ID prefix in output (default: false)
- `--quiet` (-q): Suppress connection status message (default: false)

### Example

```bash
# Check system requirements
./oyster-cvm doctor
# Sample output:
[INFO] Docker is installed ✓
[INFO] Nix is installed ✓

# Build an oyster cvm image
./oyster-cvm build \
  --platform amd64 \
  --docker-compose ./docker-compose.yml \
  --docker-images ./image1.tar ./image2.tar \
  --output ./result
# Generates a folder "result" with files
# image.eif  log.txt  pcr.json

# Upload image to IPFS using Pinata
./oyster-cvm upload --file ./result/image.eif
# Sample output:
[INFO] Successfully uploaded to Pinata: https://gateway.pinata.cloud/ipfs/Qm...


# Deploy an encalve
./oyster-cvm deploy \
  --image-url "ipfs://Qm..." \
  --region "us-east-1" \
  --wallet-private-key "your-private-key" \
  --operator "0x..." \
  --instance-type "m5a.2xlarge" \
  --duration-in-minutes 60 \
  --bandwidth 200 \
  --job-name "my-oyster-job" \
  --debug

# Sample output:
[INFO] Starting deployment...
[INFO] Total cost: 0.15 USDC
[INFO] Total rate: 0.000045 ETH/hour
[INFO] Approving USDC spend...
[INFO] USDC approval transaction: 0x3cc...e70
[INFO] Job creation transaction: 0x38b...008
[INFO] Transaction successful! Waiting 3 minutes for job initialization...
[INFO] Transaction events processed...
[INFO] Job created with ID: 0x000...37a
[INFO] Waiting for enclave to start...
[INFO] Checking for IP address...
[INFO] Found IP address: 192.168.1.100
[INFO] TCP connection established successfully
[INFO] Attestation check successful
[INFO] Enclave is ready! IP address: 192.168.1.100

# Verify an enclave
./oyster-cvm verify \
  --enclave-ip 192.168.1.100 \
  --pcr0 pcr0_value \
  --pcr1 pcr1_value \
  --pcr2 pcr2_value \

# Sample output:
[INFO] Connecting to attestation endpoint: http://192.168.1.100:1300/attestation/raw
[INFO] Successfully fetched attestation document
[INFO] Root public key: <hex-encoded-key>
[INFO] Enclave public key: <hex-encoded-key>
[INFO] Verification successful ✓

# Stream logs from an enclave
./oyster-cvm logs --ip 192.168.1.100

# Stream logs with additional options
./oyster-cvm logs \
  --ip 192.168.1.100 \
  --start-from abc123 \
  --with-log-id \
  --quiet
```

## License

This project is licensed under the Apache License, Version 2.0. See [LICENSE.txt](./LICENSE.txt).
