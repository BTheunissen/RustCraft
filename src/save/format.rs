//! Explicit little-endian progress format with a corruption checksum.

use std::io;

use glam::Vec3;

use crate::game::{
    CRATE_SLOTS, GameMode, GameSnapshot, INVENTORY_SLOTS, Item, ItemStack, MAX_CRATES,
    MAX_SAVED_EDITS, MAX_SAVED_PICKUPS, SavedCrate, SavedPickup, SavedPlayer, Tool,
};
use crate::player::SpeedMultiplier;
use crate::world::{Block, BlockEdit};

const MAGIC: &[u8; 8] = b"RUSTCRFT";
const FORMAT_VERSION: u32 = 4;
const LEGACY_FORMAT_VERSION: u32 = 3;
// Increment deliberately if seed-only generation becomes incompatible with these edits.
const GENERATION_VERSION: u32 = 2;
const HEADER_BYTES: usize = 32;
const EDIT_BYTES: usize = 13;
const PICKUP_BYTES: usize = 32;
const CRATE_BYTES: usize = 12 + CRATE_SLOTS * 4;
#[cfg(test)]
const STACK_BYTES: usize = 4;
pub(super) const MAX_SAVE_BYTES: usize = HEADER_BYTES
    + 1024
    + MAX_SAVED_EDITS * EDIT_BYTES
    + MAX_SAVED_PICKUPS * PICKUP_BYTES
    + MAX_CRATES * CRATE_BYTES;

pub(super) fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub(super) fn encode(snapshot: &GameSnapshot) -> io::Result<Vec<u8>> {
    snapshot.validate().map_err(invalid)?;
    let mut payload = Vec::with_capacity(
        256 + snapshot.edits.len() * EDIT_BYTES + snapshot.pickups.len() * PICKUP_BYTES,
    );
    payload.extend_from_slice(&snapshot.seed.to_le_bytes());
    payload.push(match snapshot.mode {
        GameMode::Creative => 0,
        GameMode::Survival => 1,
    });
    put_vec3(&mut payload, snapshot.player.position);
    put_float(&mut payload, snapshot.player.yaw);
    put_float(&mut payload, snapshot.player.pitch);
    payload.extend_from_slice(&[
        u8::from(snapshot.player.flying),
        u8::from(snapshot.headlamp),
        snapshot.speed as u8,
        snapshot.selected_slot as u8,
    ]);
    for slot in snapshot.slots {
        put_stack(&mut payload, slot);
    }
    payload.extend_from_slice(&(snapshot.edits.len() as u32).to_le_bytes());
    for edit in &snapshot.edits {
        for coordinate in edit.position {
            payload.extend_from_slice(&coordinate.to_le_bytes());
        }
        payload.push(edit.block as u8);
    }
    payload.extend_from_slice(&(snapshot.pickups.len() as u32).to_le_bytes());
    for pickup in &snapshot.pickups {
        put_vec3(&mut payload, pickup.position);
        put_stack(
            &mut payload,
            Some(ItemStack {
                item: pickup.item,
                count: pickup.count,
            }),
        );
        put_vec3(&mut payload, pickup.velocity);
        put_float(&mut payload, pickup.age);
    }
    payload.extend_from_slice(&(snapshot.crates.len() as u32).to_le_bytes());
    for saved in &snapshot.crates {
        for coordinate in saved.position {
            payload.extend_from_slice(&coordinate.to_le_bytes());
        }
        for stack in saved.slots {
            put_stack(&mut payload, stack);
        }
    }
    payload.extend_from_slice(&snapshot.time_of_day.to_le_bytes());
    let mut bytes = Vec::with_capacity(HEADER_BYTES + payload.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    bytes.extend_from_slice(&GENERATION_VERSION.to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&checksum(&payload).to_le_bytes());
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

pub(super) fn decode(bytes: &[u8]) -> io::Result<GameSnapshot> {
    if bytes.len() > MAX_SAVE_BYTES {
        return Err(invalid("local save is too large"));
    }
    let mut reader = Reader { bytes };
    if &reader.array::<8>()? != MAGIC {
        return Err(invalid("not a RustCraft save"));
    }
    let version = reader.u32()?;
    if ![LEGACY_FORMAT_VERSION, FORMAT_VERSION].contains(&version) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "unsupported RustCraft save version; back up the slot, then confirm a new development world in the menu",
        ));
    }
    if reader.u32()? != GENERATION_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "saved terrain generation version is incompatible; back up the slot, then confirm a new development world in the menu",
        ));
    }
    let length = reader.u64()?;
    let expected_checksum = reader.u64()?;
    if length != reader.bytes.len() as u64 || checksum(reader.bytes) != expected_checksum {
        return Err(invalid("saved world is incomplete or corrupt"));
    }
    let seed = reader.u64()?;
    let mode = match reader.u8()? {
        0 => GameMode::Creative,
        1 => GameMode::Survival,
        _ => return Err(invalid("unknown saved game mode")),
    };
    let player = SavedPlayer {
        position: reader.vec3()?,
        yaw: reader.float()?,
        pitch: reader.float()?,
        flying: reader.boolean()?,
    };
    let headlamp = reader.boolean()?;
    let speed = match reader.u8()? {
        1 => SpeedMultiplier::Normal,
        2 => SpeedMultiplier::Double,
        4 => SpeedMultiplier::Quadruple,
        8 => SpeedMultiplier::Octuple,
        16 => SpeedMultiplier::Sixteen,
        _ => return Err(invalid("unknown saved movement speed")),
    };
    let selected_slot = usize::from(reader.u8()?);
    let mut slots = [None; INVENTORY_SLOTS];
    for slot in &mut slots {
        *slot = reader.stack()?;
    }
    let count = reader.count(MAX_SAVED_EDITS, EDIT_BYTES)?;
    let mut edits = Vec::with_capacity(count);
    for _ in 0..count {
        edits.push(BlockEdit {
            position: [reader.i32()?, reader.i32()?, reader.i32()?],
            block: reader.block()?,
        });
    }
    let count = reader.count(MAX_SAVED_PICKUPS, PICKUP_BYTES)?;
    let mut pickups = Vec::with_capacity(count);
    for _ in 0..count {
        let position = reader.vec3()?;
        let stack = reader
            .stack()?
            .ok_or_else(|| invalid("empty saved pickup"))?;
        pickups.push(SavedPickup {
            position,
            item: stack.item,
            count: stack.count,
            velocity: reader.vec3()?,
            age: reader.float()?,
        });
    }
    let count = reader.count(MAX_CRATES, CRATE_BYTES)?;
    let mut crates = Vec::with_capacity(count);
    for _ in 0..count {
        let position = [reader.i32()?, reader.i32()?, reader.i32()?];
        let mut slots = [None; CRATE_SLOTS];
        for slot in &mut slots {
            *slot = reader.stack()?;
        }
        crates.push(SavedCrate { position, slots });
    }
    let time_of_day = if version == LEGACY_FORMAT_VERSION {
        crate::game::WorldTime::default().hour()
    } else {
        f64::from_le_bytes(reader.array()?)
    };
    if !reader.bytes.is_empty() {
        return Err(invalid("unexpected data after saved world"));
    }
    let snapshot = GameSnapshot {
        seed,
        mode,
        player,
        headlamp,
        speed,
        time_of_day,
        edits,
        slots,
        selected_slot,
        pickups,
        crates,
    };
    snapshot.validate().map_err(invalid)?;
    Ok(snapshot)
}

fn put_float(bytes: &mut Vec<u8>, value: f32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_stack(bytes: &mut Vec<u8>, stack: Option<ItemStack>) {
    let (tag, id, count) = match stack {
        None => (0, 0, 0_u16),
        Some(ItemStack {
            item: Item::Block(block),
            count,
        }) => (1, block as u8, count),
        Some(ItemStack {
            item: Item::Tool(tool),
            count,
        }) => (2, tool as u8, count),
    };
    bytes.extend_from_slice(&[tag, id]);
    bytes.extend_from_slice(&count.to_le_bytes());
}

fn put_vec3(bytes: &mut Vec<u8>, value: Vec3) {
    for coordinate in value.to_array() {
        put_float(bytes, coordinate);
    }
}

fn checksum(bytes: &[u8]) -> u64 {
    // FNV-1a detects accidental damage, not malicious modification.
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    })
}

struct Reader<'a> {
    bytes: &'a [u8],
}

impl Reader<'_> {
    fn array<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let (value, rest) = self
            .bytes
            .split_at_checked(N)
            .ok_or_else(|| invalid("saved world is truncated"))?;
        let mut array = [0; N];
        array.copy_from_slice(value);
        self.bytes = rest;
        Ok(array)
    }

    fn u8(&mut self) -> io::Result<u8> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> io::Result<u16> {
        self.array().map(u16::from_le_bytes)
    }

    fn u32(&mut self) -> io::Result<u32> {
        self.array().map(u32::from_le_bytes)
    }

    fn u64(&mut self) -> io::Result<u64> {
        self.array().map(u64::from_le_bytes)
    }

    fn i32(&mut self) -> io::Result<i32> {
        self.array().map(i32::from_le_bytes)
    }

    fn float(&mut self) -> io::Result<f32> {
        self.array().map(f32::from_le_bytes)
    }

    fn vec3(&mut self) -> io::Result<Vec3> {
        Ok(Vec3::new(self.float()?, self.float()?, self.float()?))
    }

    fn boolean(&mut self) -> io::Result<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(invalid("invalid saved boolean")),
        }
    }

    fn block(&mut self) -> io::Result<Block> {
        Block::ALL
            .get(usize::from(self.u8()?))
            .copied()
            .ok_or_else(|| invalid("unknown saved block material"))
    }

    fn stack(&mut self) -> io::Result<Option<ItemStack>> {
        let tag = self.u8()?;
        let id = self.u8()?;
        let count = self.u16()?;
        let item = match tag {
            0 if id == 0 && count == 0 => return Ok(None),
            1 => Item::Block(
                *Block::ALL
                    .get(usize::from(id))
                    .ok_or_else(|| invalid("unknown saved block item"))?,
            ),
            2 => Item::Tool(
                *Tool::ALL
                    .get(usize::from(id))
                    .ok_or_else(|| invalid("unknown saved tool item"))?,
            ),
            _ => return Err(invalid("unknown or malformed saved item tag")),
        };
        let stack = ItemStack { item, count };
        if !stack.is_valid() {
            return Err(invalid("invalid saved item stack"));
        }
        Ok(Some(stack))
    }

    fn count(&mut self, maximum: usize, record_bytes: usize) -> io::Result<usize> {
        let count = self.u32()? as usize;
        if count > maximum || count * record_bytes > self.bytes.len() {
            return Err(invalid("invalid saved record count"));
        }
        Ok(count)
    }
}

#[cfg(test)]
mod item_tests;
#[cfg(test)]
mod storage_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod time_tests;
