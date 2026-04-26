import { openDB, type DBSchema, type IDBPDatabase } from 'idb';
import { v4 as uuid } from 'uuid';
import type { Stamp } from '../types';

const DB_NAME = 'timbrapp';
const DB_VERSION = 1;
const STORE = 'stamps';

interface TimbrappDB extends DBSchema {
	stamps: {
		key: string;
		value: Stamp;
		indexes: { 'by-createdAt': number };
	};
}

let dbPromise: Promise<IDBPDatabase<TimbrappDB>> | null = null;

function getDB(): Promise<IDBPDatabase<TimbrappDB>> {
	if (!dbPromise) {
		dbPromise = openDB<TimbrappDB>(DB_NAME, DB_VERSION, {
			upgrade(db) {
				if (!db.objectStoreNames.contains(STORE)) {
					const store = db.createObjectStore(STORE, { keyPath: 'id' });
					store.createIndex('by-createdAt', 'createdAt');
				}
			}
		});
	}
	return dbPromise;
}

/** List all stamps, oldest first. */
export async function listStamps(): Promise<Stamp[]> {
	const db = await getDB();
	const tx = db.transaction(STORE, 'readonly');
	const all = await tx.store.index('by-createdAt').getAll();
	await tx.done;
	return all;
}

export async function getStamp(id: string): Promise<Stamp | undefined> {
	const db = await getDB();
	return db.get(STORE, id);
}

/**
 * Persist a stamp from a PNG `Blob`. Returns the stored `Stamp` (with
 * generated id + timestamp).
 */
export async function addStamp(name: string, blob: Blob): Promise<Stamp> {
	if (blob.type && blob.type !== 'image/png') {
		throw new Error(`Only PNG stamps are supported (got ${blob.type})`);
	}
	const stamp: Stamp = {
		id: uuid(),
		name: name || 'stamp',
		blob,
		createdAt: Date.now()
	};
	const db = await getDB();
	await db.add(STORE, stamp);
	return stamp;
}

export async function deleteStamp(id: string): Promise<void> {
	const db = await getDB();
	await db.delete(STORE, id);
}

export async function countStamps(): Promise<number> {
	const db = await getDB();
	return db.count(STORE);
}
