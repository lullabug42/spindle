import { ServiceTopo } from "@/types/service.types";
import { sha256 } from "@noble/hashes/sha2.js";
import { bytesToHex, utf8ToBytes } from "@noble/hashes/utils.js";
import stringify from "canonical-json";

export function toHash(topo: ServiceTopo) {
    const configBytes = utf8ToBytes(stringify(topo));
    return bytesToHex(sha256(configBytes));
}