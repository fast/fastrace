This directory contains results of the `compare` benchmark runs in different AWS EC2 VMs (Ubuntu Server 20.04 LTS).

## VM descriptions

| Instance Type | Architecture | Microarchitecture     | vCPU | Memory (GiB) |
|---------------|--------------|-----------------------|------|--------------|
| m4.2xlarge    | x86-64       | Intel Broadwell       | 8    | 32           |
| m5.2xlarge    | x86-64       | Intel Skylake         | 8    | 32           |
| m5a.2xlarge   | x86-64       | AMD Zen               | 8    | 32           |
| m6g.2xlarge   | arm64        | AWS Graviton2 (ARMv8) | 8    | 32           |

See more details in the [AWS website](https://aws.amazon.com/ec2/instance-types).

## Generate Results in a fresh VM

```shell
sudo apt update
sudo apt install build-essential libssl-dev pkg-config -y
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
cargo install --version=1.0.0-alpha3 cargo-criterion
git clone https://github.com/fastracelabs/fastrace.git
cd fastrace
cargo criterion compare --message-format=json | grep "benchmark-complete" > compare-xxx.txt
```
