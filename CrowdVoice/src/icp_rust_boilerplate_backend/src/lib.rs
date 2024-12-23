#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Proposal {
    id: u64,
    title: String,
    description: String,
    votes_for: u64,
    votes_against: u64,
    creator: String,
    created_at: u64,
    updated_at: Option<u64>,
}

impl Storable for Proposal {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Proposal {
    const MAX_SIZE: u32 = 2048;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    static PROPOSALS: RefCell<StableBTreeMap<u64, Proposal, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
    ));
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct ProposalPayload {
    title: String,
    description: String,
}

#[ic_cdk::update]
fn add_proposal(payload: ProposalPayload) -> Proposal {
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("Cannot increment ID counter");

    let proposal = Proposal {
        id,
        title: payload.title,
        description: payload.description,
        votes_for: 0,
        votes_against: 0,
        creator: ic_cdk::caller().to_string(),
        created_at: time(),
        updated_at: None,
    };

    PROPOSALS.with(|map| map.borrow_mut().insert(id, proposal.clone()));
    proposal
}

#[ic_cdk::query]
fn get_proposal(id: u64) -> Result<Proposal, String> {
    PROPOSALS.with(|map| map.borrow().get(&id).cloned())
        .ok_or_else(|| format!("Proposal with ID {} not found", id))
}

#[ic_cdk::update]
fn vote(id: u64, vote_for: bool) -> Result<Proposal, String> {
    PROPOSALS.with(|map| {
        let mut proposals = map.borrow_mut();
        if let Some(proposal) = proposals.get_mut(&id) {
            if vote_for {
                proposal.votes_for += 1;
            } else {
                proposal.votes_against += 1;
            }
            proposal.updated_at = Some(time());
            Ok(proposal.clone())
        } else {
            Err(format!("Proposal with ID {} not found", id))
        }
    })
}

#[ic_cdk::update]
fn delete_proposal(id: u64) -> Result<Proposal, String> {
    PROPOSALS.with(|map| map.borrow_mut().remove(&id))
        .ok_or_else(|| format!("Proposal with ID {} not found", id))
}

#[derive(candid::CandidType, Serialize, Deserialize)]
enum Error {
    NotFound { msg: String },
}

// Generates the candid interface for this canister
ic_cdk::export_candid!();
