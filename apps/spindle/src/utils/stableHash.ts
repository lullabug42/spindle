
import { sha256 } from "@noble/hashes/sha2.js";
import { bytesToHex, utf8ToBytes } from "@noble/hashes/utils.js";
import stringify from "canonical-json";

export function toHash<T>(data: T) {
    const dataBytes = utf8ToBytes(stringify(data));
    return bytesToHex(sha256(dataBytes));
}