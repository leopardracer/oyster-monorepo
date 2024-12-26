import { ethers, upgrades, run } from 'hardhat';
import { Contract } from 'ethers';
import * as fs from 'fs';

import yargs from 'yargs';
import { hideBin } from 'yargs/helpers';

import { upgrade as upgradeUtil } from './Upgrade';

import * as configI from './config.json';
const config = configI as any;

export async function deploy(): Promise<Contract> {
  const AttestationVerifier = await ethers.getContractFactory('AttestationVerifier');

  let chainId = (await ethers.provider.getNetwork()).chainId.toString();
  console.log("Chain Id:", chainId);

  const chainConfig = config[chainId];

  var addresses: { [key: string]: { [key: string]: string } } = {};
  if (fs.existsSync('address.json')) {
    addresses = JSON.parse(fs.readFileSync('address.json', 'utf8'));
  }

  if (addresses[chainId] === undefined) {
    addresses[chainId] = {};
  }

  if (addresses[chainId]['AttestationVerifier'] !== undefined) {
    console.log("Existing deployment:", addresses[chainId]['AttestationVerifier']);
    return AttestationVerifier.attach(addresses[chainId]['AttestationVerifier']);
  }

  let enclaveImages = [];
  for (let i = 0; i < chainConfig.enclaves.whitelistedImages.length; i++) {
    if (chainConfig.enclaves.whitelistedImages[i].PCR === undefined) {
      throw new Error(`Image ${i}: PCR not defined for image`);
    }
    if (chainConfig.enclaves.whitelistedImages[i].PCR.PCR0.length !== 96) {
      throw new Error(`Image ${i}: PCR0 length is not 96`);
    }
    if (chainConfig.enclaves.whitelistedImages[i].PCR.PCR1.length !== 96) {
      throw new Error(`Image ${i}: PCR1 length is not 96`);
    }
    if (chainConfig.enclaves.whitelistedImages[i].PCR.PCR2.length !== 96) {
      throw new Error(`Image ${i}: PCR2 length is not 96`);
    }
    const image = {
      PCR0: "0x" + chainConfig.enclaves.whitelistedImages[i].PCR.PCR0,
      PCR1: "0x" + chainConfig.enclaves.whitelistedImages[i].PCR.PCR1,
      PCR2: "0x" + chainConfig.enclaves.whitelistedImages[i].PCR.PCR2,
    };
    enclaveImages.push(image);
  }

  let enclaveKeys = [];
  for (let i = 0; i < chainConfig.enclaves.whitelistedImages.length; i++) {
    if (chainConfig.enclaves.whitelistedImages[i].enclaveKey === undefined) {
      throw new Error(`Image ${i}: Enclave key not defined for image`);
    }
    enclaveKeys.push(chainConfig.enclaves.whitelistedImages[i].enclaveKey);
  }

  let admin = chainConfig.admin;

  let attestationVerifier = await upgrades.deployProxy(AttestationVerifier, [enclaveImages, enclaveKeys, admin], { kind: "uups" });

  console.log("Deployed addr:", await attestationVerifier.getAddress());

  addresses[chainId]['AttestationVerifier'] = await attestationVerifier.getAddress();

  fs.writeFileSync('address.json', JSON.stringify(addresses, null, 2), 'utf8');

  return attestationVerifier;
}

export async function upgrade() {
  await upgradeUtil('AttestationVerifier', 'AttestationVerifier', []);
}

export async function verify() {
  let chainId = (await ethers.provider.getNetwork()).chainId.toString();
  console.log("Chain Id:", chainId);

  var addresses: { [key: string]: { [key: string]: string } } = {};
  if (fs.existsSync('address.json')) {
    addresses = JSON.parse(fs.readFileSync('address.json', 'utf8'));
  }

  if (addresses[chainId] === undefined || addresses[chainId]['AttestationVerifier'] === undefined) {
    throw new Error("Attestation Verifier not deployed");
  }

  await run("verify:verify", {
    address: addresses[chainId]['AttestationVerifier'],
    constructorArguments: []
  });

  console.log("Attestation Verifier verified");
}

yargs(hideBin(process.argv))
  .command({
    command: 'deploy',
    describe: 'Deploy the contract',
    handler: (_argv) => deploy().then(),
  })
  .command({
    command: 'upgrade',
    describe: 'Upgrade the contract',
    handler: (_argv) => upgrade().then(),
  })
  .command({
    command: 'verify',
    describe: 'Verify the contract',
    handler: (_argv) => verify().then(),
  })
  .demandCommand(1, 'expected a command from deploy, upgrade or verify')
  .strict()
  .help()
  .argv;
