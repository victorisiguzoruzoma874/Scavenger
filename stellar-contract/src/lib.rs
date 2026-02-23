#![no_std]

mod types;

pub use types::{Material, ParticipantRole, RecyclingStats, WasteType};

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Participant {
    pub address: Address,
    pub role: ParticipantRole,
    pub registered_at: u64,
}

/// Represents a manufacturer incentive program for recycling specific waste types
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Incentive {
    /// Unique identifier for the incentive
    pub id: u64,
    /// Type of waste this incentive applies to
    pub waste_type: WasteType,
    /// Reward amount per unit (in tokens)
    pub reward: u128,
    /// Maximum waste amount eligible for this incentive (in grams)
    pub max_waste_amount: u128,
    /// Address of the manufacturer offering the incentive
    pub rewarder: Address,
    /// Whether this incentive is currently active
    pub is_active: bool,
    /// Timestamp when the incentive was created
    pub created_at: u64,
}

#[contract]
pub struct ScavengerContract;

#[contractimpl]
impl ScavengerContract {
    /// Register a new participant with a specific role
    pub fn register_participant(
        env: Env,
        address: Address,
        role: ParticipantRole,
    ) -> Participant {
        address.require_auth();

        let participant = Participant {
            address: address.clone(),
            role,
            registered_at: env.ledger().timestamp(),
        };

        // Store participant in contract storage
        let key = (address.clone(),);
        env.storage().instance().set(&key, &participant);

        participant
    }

    /// Store a waste record by ID
    /// Internal helper function for efficient waste storage
    fn set_waste(env: &Env, waste_id: u64, material: &Material) {
        let key = ("waste", waste_id);
        env.storage().instance().set(&key, material);
    }

    /// Retrieve a waste record by ID
    /// Returns None if waste doesn't exist
    fn get_waste(env: &Env, waste_id: u64) -> Option<Material> {
        let key = ("waste", waste_id);
        env.storage().instance().get(&key)
    }

    /// Check if a waste record exists
    pub fn waste_exists(env: Env, waste_id: u64) -> bool {
        let key = ("waste", waste_id);
        env.storage().instance().has(&key)
    }

    /// Get the total count of waste records
    fn get_waste_count(env: &Env) -> u64 {
        env.storage().instance().get(&("waste_count",)).unwrap_or(0)
    }

    /// Increment and return the next waste ID
    fn next_waste_id(env: &Env) -> u64 {
        let count = Self::get_waste_count(env);
        let next_id = count + 1;
        env.storage().instance().set(&("waste_count",), &next_id);
        next_id
    }

    /// Get the total count of incentive records
    fn get_incentive_count(env: &Env) -> u64 {
        env.storage().instance().get(&("incentive_count",)).unwrap_or(0)
    }

    /// Increment and return the next incentive ID
    fn next_incentive_id(env: &Env) -> u64 {
        let count = Self::get_incentive_count(env);
        let next_id = count + 1;
        env.storage().instance().set(&("incentive_count",), &next_id);
        next_id
    }

    /// Store an incentive record by ID
    /// Internal helper function for efficient incentive storage
    fn set_incentive(env: &Env, incentive_id: u64, incentive: &Incentive) {
        let key = ("incentive", incentive_id);
        env.storage().instance().set(&key, incentive);
    }

    /// Retrieve an incentive record by ID
    /// Returns None if incentive doesn't exist
    fn get_incentive(env: &Env, incentive_id: u64) -> Option<Incentive> {
        let key = ("incentive", incentive_id);
        env.storage().instance().get(&key)
    }

    /// Check if an incentive record exists
    pub fn incentive_exists(env: Env, incentive_id: u64) -> bool {
        let key = ("incentive", incentive_id);
        env.storage().instance().has(&key)
    }

    /// Create a new manufacturer incentive program
    /// Only manufacturers can create incentives
    pub fn create_incentive(
        env: Env,
        waste_type: WasteType,
        reward: u128,
        max_waste_amount: u128,
        rewarder: Address,
    ) -> Incentive {
        rewarder.require_auth();

        // Validate rewarder is a registered manufacturer
        let rewarder_key = (rewarder.clone(),);
        let participant: Participant = env
            .storage()
            .instance()
            .get(&rewarder_key)
            .expect("Rewarder not registered");

        if !participant.role.can_manufacture() {
            panic!("Only manufacturers can create incentives");
        }

        // Validate input values
        if reward == 0 {
            panic!("Reward must be greater than zero");
        }

        if max_waste_amount == 0 {
            panic!("Max waste amount must be greater than zero");
        }

        // Get next incentive ID
        let incentive_id = Self::next_incentive_id(&env);

        // Create incentive
        let incentive = Incentive {
            id: incentive_id,
            waste_type,
            reward,
            max_waste_amount,
            rewarder: rewarder.clone(),
            is_active: true,
            created_at: env.ledger().timestamp(),
        };

        // Store incentive
        Self::set_incentive(&env, incentive_id, &incentive);

        incentive
    }

    /// Get incentive by ID
    pub fn get_incentive_by_id(env: Env, incentive_id: u64) -> Option<Incentive> {
        Self::get_incentive(&env, incentive_id)
    }

    /// Update incentive active status
    /// Only the rewarder can update their incentive
    pub fn update_incentive_status(
        env: Env,
        incentive_id: u64,
        is_active: bool,
    ) -> Incentive {
        let mut incentive: Incentive = Self::get_incentive(&env, incentive_id)
            .expect("Incentive not found");

        // Require auth from the rewarder
        incentive.rewarder.require_auth();

        incentive.is_active = is_active;
        Self::set_incentive(&env, incentive_id, &incentive);

        incentive
    }

    /// Calculate reward for a given waste amount based on an incentive
    /// Returns the reward amount, respecting max_waste_amount and is_active status
    pub fn calculate_incentive_reward(
        env: Env,
        incentive_id: u64,
        waste_amount: u64,
    ) -> u128 {
        let incentive: Incentive = Self::get_incentive(&env, incentive_id)
            .expect("Incentive not found");

        // Check if incentive is active
        if !incentive.is_active {
            return 0;
        }

        // Convert waste_amount to u128 for calculation
        let waste_amount_u128 = waste_amount as u128;

        // Cap waste amount at max_waste_amount
        let eligible_amount = if waste_amount_u128 > incentive.max_waste_amount {
            incentive.max_waste_amount
        } else {
            waste_amount_u128
        };

        // Calculate reward using checked arithmetic
        // reward_per_gram * eligible_amount
        eligible_amount
            .checked_mul(incentive.reward)
            .and_then(|result| result.checked_div(1000)) // Assuming reward is per kg, divide by 1000 for grams
            .expect("Overflow in reward calculation")
    }

    /// Get all incentives for a specific waste type
    pub fn get_incentives_by_waste_type(
        env: Env,
        waste_type: WasteType,
    ) -> soroban_sdk::Vec<Incentive> {
        let mut results = soroban_sdk::Vec::new(&env);
        let count = Self::get_incentive_count(&env);

        for i in 1..=count {
            if let Some(incentive) = Self::get_incentive(&env, i) {
                if incentive.waste_type == waste_type {
                    results.push_back(incentive);
                }
            }
        }

        results
    }

    /// Get all active incentives
    pub fn get_active_incentives(env: Env) -> soroban_sdk::Vec<Incentive> {
        let mut results = soroban_sdk::Vec::new(&env);
        let count = Self::get_incentive_count(&env);

        for i in 1..=count {
            if let Some(incentive) = Self::get_incentive(&env, i) {
                if incentive.is_active {
                    results.push_back(incentive);
                }
            }
        }

        results
    }

    /// Get participant information
    pub fn get_participant(env: Env, address: Address) -> Option<Participant> {
        let key = (address,);
        env.storage().instance().get(&key)
    }

    /// Update participant role
    pub fn update_role(env: Env, address: Address, new_role: ParticipantRole) -> Participant {
        address.require_auth();

        let key = (address.clone(),);
        let mut participant: Participant = env
            .storage()
            .instance()
            .get(&key)
            .expect("Participant not found");

        participant.role = new_role;
        env.storage().instance().set(&key, &participant);

        participant
    }

    /// Validate if a participant can perform a specific action
    pub fn can_collect(env: Env, address: Address) -> bool {
        let key = (address,);
        if let Some(participant) = env.storage().instance().get::<_, Participant>(&key) {
            participant.role.can_collect_materials()
        } else {
            false
        }
    }

    /// Validate if a participant can manufacture
    pub fn can_manufacture(env: Env, address: Address) -> bool {
        let key = (address,);
        if let Some(participant) = env.storage().instance().get::<_, Participant>(&key) {
            participant.role.can_manufacture()
        } else {
            false
        }
    }

    /// Submit a new material for recycling
    pub fn submit_material(
        env: Env,
        waste_type: WasteType,
        weight: u64,
        submitter: Address,
        description: String,
    ) -> Material {
        submitter.require_auth();

        // Get next waste ID using the new storage system
        let waste_id = Self::next_waste_id(&env);

        // Create material
        let material = Material::new(
            waste_id,
            waste_type,
            weight,
            submitter.clone(),
            env.ledger().timestamp(),
            description,
        );

        // Store waste using the new storage system
        Self::set_waste(&env, waste_id, &material);

        // Update stats
        let mut stats: RecyclingStats = env
            .storage()
            .instance()
            .get(&("stats", submitter.clone()))
            .unwrap_or_else(|| RecyclingStats::new(submitter.clone()));
        
        stats.record_submission(&material);
        env.storage().instance().set(&("stats", submitter), &stats);

        material
    }

    /// Batch submit multiple materials for recycling
    /// More efficient than individual submissions
    pub fn submit_materials_batch(
        env: Env,
        materials: soroban_sdk::Vec<(WasteType, u64, String)>,
        submitter: Address,
    ) -> soroban_sdk::Vec<Material> {
        submitter.require_auth();

        let mut results = soroban_sdk::Vec::new(&env);
        let timestamp = env.ledger().timestamp();

        // Get or create stats once
        let mut stats: RecyclingStats = env
            .storage()
            .instance()
            .get(&("stats", submitter.clone()))
            .unwrap_or_else(|| RecyclingStats::new(submitter.clone()));

        // Process each material
        for item in materials.iter() {
            let (waste_type, weight, description) = item;
            let waste_id = Self::next_waste_id(&env);

            let material = Material::new(
                waste_id,
                waste_type,
                weight,
                submitter.clone(),
                timestamp,
                description,
            );

            Self::set_waste(&env, waste_id, &material);
            stats.record_submission(&material);
            results.push_back(material);
        }

        // Update stats once at the end
        env.storage().instance().set(&("stats", submitter), &stats);

        results
    }

    /// Get material by ID (alias for get_waste for backward compatibility)
    pub fn get_material(env: Env, material_id: u64) -> Option<Material> {
        Self::get_waste(&env, material_id)
    }

    /// Get waste by ID
    pub fn get_waste_by_id(env: Env, waste_id: u64) -> Option<Material> {
        Self::get_waste(&env, waste_id)
    }

    /// Get multiple wastes by IDs (batch retrieval)
    pub fn get_wastes_batch(env: Env, waste_ids: soroban_sdk::Vec<u64>) -> soroban_sdk::Vec<Option<Material>> {
        let mut results = soroban_sdk::Vec::new(&env);
        
        for waste_id in waste_ids.iter() {
            results.push_back(Self::get_waste(&env, waste_id));
        }
        
        results
    }

    /// Verify a material submission (only recyclers can verify)
    pub fn verify_material(env: Env, material_id: u64, verifier: Address) -> Material {
        verifier.require_auth();

        // Check if verifier is a recycler
        let verifier_key = (verifier.clone(),);
        let participant: Participant = env
            .storage()
            .instance()
            .get(&verifier_key)
            .expect("Verifier not registered");

        if !participant.role.can_process_recyclables() {
            panic!("Only recyclers can verify materials");
        }

        // Get and verify material using new storage system
        let mut material: Material = Self::get_waste(&env, material_id)
            .expect("Material not found");

        material.verify();
        Self::set_waste(&env, material_id, &material);

        // Update submitter stats
        let mut stats: RecyclingStats = env
            .storage()
            .instance()
            .get(&("stats", material.submitter.clone()))
            .unwrap_or_else(|| RecyclingStats::new(material.submitter.clone()));
        
        stats.record_verification(&material);
        env.storage().instance().set(&("stats", material.submitter.clone()), &stats);

        material
    }

    /// Batch verify multiple materials
    pub fn verify_materials_batch(
        env: Env,
        material_ids: soroban_sdk::Vec<u64>,
        verifier: Address,
    ) -> soroban_sdk::Vec<Material> {
        verifier.require_auth();

        // Check if verifier is a recycler
        let verifier_key = (verifier.clone(),);
        let participant: Participant = env
            .storage()
            .instance()
            .get(&verifier_key)
            .expect("Verifier not registered");

        if !participant.role.can_process_recyclables() {
            panic!("Only recyclers can verify materials");
        }

        let mut results = soroban_sdk::Vec::new(&env);

        for material_id in material_ids.iter() {
            if let Some(mut material) = Self::get_waste(&env, material_id) {
                material.verify();
                Self::set_waste(&env, material_id, &material);

                // Update submitter stats
                let mut stats: RecyclingStats = env
                    .storage()
                    .instance()
                    .get(&("stats", material.submitter.clone()))
                    .unwrap_or_else(|| RecyclingStats::new(material.submitter.clone()));
                
                stats.record_verification(&material);
                env.storage().instance().set(&("stats", material.submitter.clone()), &stats);

                results.push_back(material);
            }
        }

        results
    }

    /// Get recycling statistics for a participant
    pub fn get_stats(env: Env, participant: Address) -> Option<RecyclingStats> {
        env.storage().instance().get(&("stats", participant))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    #[test]
    fn test_register_participant() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        let participant = client.register_participant(&user, &ParticipantRole::Recycler);

        assert_eq!(participant.address, user);
        assert_eq!(participant.role, ParticipantRole::Recycler);
        assert!(participant.registered_at > 0);
    }

    #[test]
    fn test_get_participant() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        client.register_participant(&user, &ParticipantRole::Collector);

        let participant = client.get_participant(&user);
        assert!(participant.is_some());
        assert_eq!(participant.unwrap().role, ParticipantRole::Collector);
    }

    #[test]
    fn test_update_role() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        client.register_participant(&user, &ParticipantRole::Recycler);
        let updated = client.update_role(&user, &ParticipantRole::Manufacturer);

        assert_eq!(updated.role, ParticipantRole::Manufacturer);
    }

    #[test]
    fn test_can_collect() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let recycler = Address::generate(&env);
        let collector = Address::generate(&env);
        let manufacturer = Address::generate(&env);
        env.mock_all_auths();

        client.register_participant(&recycler, &ParticipantRole::Recycler);
        client.register_participant(&collector, &ParticipantRole::Collector);
        client.register_participant(&manufacturer, &ParticipantRole::Manufacturer);

        assert!(client.can_collect(&recycler));
        assert!(client.can_collect(&collector));
        assert!(!client.can_collect(&manufacturer));
    }

    #[test]
    fn test_can_manufacture() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let recycler = Address::generate(&env);
        let manufacturer = Address::generate(&env);
        env.mock_all_auths();

        client.register_participant(&recycler, &ParticipantRole::Recycler);
        client.register_participant(&manufacturer, &ParticipantRole::Manufacturer);

        assert!(!client.can_manufacture(&recycler));
        assert!(client.can_manufacture(&manufacturer));
    }

    #[test]
    fn test_all_role_types() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let user3 = Address::generate(&env);
        env.mock_all_auths();

        client.register_participant(&user1, &ParticipantRole::Recycler);
        client.register_participant(&user2, &ParticipantRole::Collector);
        client.register_participant(&user3, &ParticipantRole::Manufacturer);

        let p1 = client.get_participant(&user1).unwrap();
        let p2 = client.get_participant(&user2).unwrap();
        let p3 = client.get_participant(&user3).unwrap();

        assert_eq!(p1.role, ParticipantRole::Recycler);
        assert_eq!(p2.role, ParticipantRole::Collector);
        assert_eq!(p3.role, ParticipantRole::Manufacturer);
    }

    #[test]
    fn test_waste_type_storage() {
        let env = Env::default();
        
        // Test that WasteType can be stored and retrieved from storage
        let waste_types = [
            WasteType::Paper,
            WasteType::PetPlastic,
            WasteType::Plastic,
            WasteType::Metal,
            WasteType::Glass,
        ];

        for (i, waste_type) in waste_types.iter().enumerate() {
            let key = (i as u32,);
            env.storage().instance().set(&key, waste_type);
            let retrieved: WasteType = env.storage().instance().get(&key).unwrap();
            assert_eq!(retrieved, *waste_type);
        }
    }

    #[test]
    fn test_waste_type_serialization() {
        let env = Env::default();
        
        // Test all waste types can be serialized/deserialized
        let all_types = [
            WasteType::Paper,
            WasteType::PetPlastic,
            WasteType::Plastic,
            WasteType::Metal,
            WasteType::Glass,
        ];

        for waste_type in all_types.iter() {
            // Store in instance storage
            env.storage().instance().set(&("waste",), waste_type);
            let retrieved: WasteType = env.storage().instance().get(&("waste",)).unwrap();
            assert_eq!(retrieved, *waste_type);
            
            // Verify string representation
            assert!(!waste_type.as_str().is_empty());
        }
    }

    #[test]
    fn test_submit_material() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        let description = String::from_str(&env, "Plastic bottles");
        let material = client.submit_material(
            &WasteType::PetPlastic,
            &5000,
            &user,
            &description,
        );

        assert_eq!(material.id, 1);
        assert_eq!(material.waste_type, WasteType::PetPlastic);
        assert_eq!(material.weight, 5000);
        assert_eq!(material.submitter, user);
        assert!(!material.verified);
    }

    #[test]
    fn test_get_material() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        let description = String::from_str(&env, "Metal cans");
        client.submit_material(&WasteType::Metal, &3000, &user, &description);

        let material = client.get_material(&1);
        assert!(material.is_some());
        assert_eq!(material.unwrap().waste_type, WasteType::Metal);
    }

    #[test]
    fn test_verify_material() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let submitter = Address::generate(&env);
        let recycler = Address::generate(&env);
        env.mock_all_auths();

        // Register recycler
        client.register_participant(&recycler, &ParticipantRole::Recycler);

        // Submit material
        let description = String::from_str(&env, "Glass bottles");
        client.submit_material(&WasteType::Glass, &2000, &submitter, &description);

        // Verify material
        let verified = client.verify_material(&1, &recycler);
        assert!(verified.verified);
    }

    #[test]
    fn test_multiple_materials() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        // Submit multiple materials
        let desc1 = String::from_str(&env, "Paper");
        let desc2 = String::from_str(&env, "Plastic");
        let desc3 = String::from_str(&env, "Metal");

        client.submit_material(&WasteType::Paper, &1000, &user, &desc1);
        client.submit_material(&WasteType::Plastic, &2000, &user, &desc2);
        client.submit_material(&WasteType::Metal, &3000, &user, &desc3);

        // Verify all materials exist
        assert!(client.get_material(&1).is_some());
        assert!(client.get_material(&2).is_some());
        assert!(client.get_material(&3).is_some());
        assert!(client.get_material(&4).is_none());
    }

    #[test]
    fn test_stats_tracking() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        // Submit materials
        let desc = String::from_str(&env, "Test");
        client.submit_material(&WasteType::Paper, &1000, &user, &desc);
        client.submit_material(&WasteType::Plastic, &2000, &user, &desc);

        // Check stats
        let stats = client.get_stats(&user);
        assert!(stats.is_some());
        let stats = stats.unwrap();
        assert_eq!(stats.total_submissions, 2);
        assert_eq!(stats.total_weight, 3000);
    }

    #[test]
    fn test_stats_with_verification() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let submitter = Address::generate(&env);
        let recycler = Address::generate(&env);
        env.mock_all_auths();

        // Register recycler
        client.register_participant(&recycler, &ParticipantRole::Recycler);

        // Submit and verify material
        let desc = String::from_str(&env, "Metal cans");
        client.submit_material(&WasteType::Metal, &5000, &submitter, &desc);
        client.verify_material(&1, &recycler);

        // Check stats
        let stats = client.get_stats(&submitter).unwrap();
        assert_eq!(stats.total_submissions, 1);
        assert_eq!(stats.verified_submissions, 1);
        assert_eq!(stats.total_points, 250); // 5kg * 5 * 10
        assert_eq!(stats.verification_rate(), 100);
    }

    #[test]
    fn test_stats_most_submitted_type() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        let desc = String::from_str(&env, "Test");
        
        // Submit multiple plastic items
        client.submit_material(&WasteType::Plastic, &1000, &user, &desc);
        client.submit_material(&WasteType::Plastic, &2000, &user, &desc);
        client.submit_material(&WasteType::Paper, &1000, &user, &desc);

        let stats = client.get_stats(&user).unwrap();
        assert_eq!(stats.plastic_count, 2);
        assert_eq!(stats.paper_count, 1);
    }

    // Waste Storage System Tests
    #[test]
    fn test_waste_exists() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        // Check non-existent waste
        assert!(!client.waste_exists(&1));

        // Submit material
        let desc = String::from_str(&env, "Test waste");
        client.submit_material(&WasteType::Paper, &1000, &user, &desc);

        // Check existing waste
        assert!(client.waste_exists(&1));
        assert!(!client.waste_exists(&2));
    }

    #[test]
    fn test_get_waste_by_id() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        let desc = String::from_str(&env, "Glass bottles");
        client.submit_material(&WasteType::Glass, &3000, &user, &desc);

        let waste = client.get_waste_by_id(&1);
        assert!(waste.is_some());
        let waste = waste.unwrap();
        assert_eq!(waste.id, 1);
        assert_eq!(waste.waste_type, WasteType::Glass);
        assert_eq!(waste.weight, 3000);
    }

    #[test]
    fn test_get_wastes_batch() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        let desc = String::from_str(&env, "Test");
        
        // Submit multiple materials
        client.submit_material(&WasteType::Paper, &1000, &user, &desc);
        client.submit_material(&WasteType::Plastic, &2000, &user, &desc);
        client.submit_material(&WasteType::Metal, &3000, &user, &desc);

        // Batch retrieve
        let mut ids = soroban_sdk::Vec::new(&env);
        ids.push_back(1);
        ids.push_back(2);
        ids.push_back(3);
        ids.push_back(99); // Non-existent

        let results = client.get_wastes_batch(&ids);
        assert_eq!(results.len(), 4);
        assert!(results.get(0).unwrap().is_some());
        assert!(results.get(1).unwrap().is_some());
        assert!(results.get(2).unwrap().is_some());
        assert!(results.get(3).unwrap().is_none());
    }

    #[test]
    fn test_submit_materials_batch() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        // Prepare batch materials
        let mut materials = soroban_sdk::Vec::new(&env);
        materials.push_back((
            WasteType::Paper,
            1000u64,
            String::from_str(&env, "Paper batch"),
        ));
        materials.push_back((
            WasteType::Plastic,
            2000u64,
            String::from_str(&env, "Plastic batch"),
        ));
        materials.push_back((
            WasteType::Metal,
            3000u64,
            String::from_str(&env, "Metal batch"),
        ));

        // Submit batch
        let results = client.submit_materials_batch(&materials, &user);
        
        assert_eq!(results.len(), 3);
        assert_eq!(results.get(0).unwrap().waste_type, WasteType::Paper);
        assert_eq!(results.get(1).unwrap().waste_type, WasteType::Plastic);
        assert_eq!(results.get(2).unwrap().waste_type, WasteType::Metal);

        // Verify all were stored
        assert!(client.waste_exists(&1));
        assert!(client.waste_exists(&2));
        assert!(client.waste_exists(&3));

        // Check stats were updated
        let stats = client.get_stats(&user).unwrap();
        assert_eq!(stats.total_submissions, 3);
        assert_eq!(stats.total_weight, 6000);
    }

    #[test]
    fn test_verify_materials_batch() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let submitter = Address::generate(&env);
        let recycler = Address::generate(&env);
        env.mock_all_auths();

        // Register recycler
        client.register_participant(&recycler, &ParticipantRole::Recycler);

        // Submit multiple materials
        let desc = String::from_str(&env, "Test");
        client.submit_material(&WasteType::Paper, &1000, &submitter, &desc);
        client.submit_material(&WasteType::Plastic, &2000, &submitter, &desc);
        client.submit_material(&WasteType::Metal, &3000, &submitter, &desc);

        // Batch verify
        let mut ids = soroban_sdk::Vec::new(&env);
        ids.push_back(1);
        ids.push_back(2);
        ids.push_back(3);

        let results = client.verify_materials_batch(&ids, &recycler);
        
        assert_eq!(results.len(), 3);
        assert!(results.get(0).unwrap().verified);
        assert!(results.get(1).unwrap().verified);
        assert!(results.get(2).unwrap().verified);

        // Check stats were updated
        let stats = client.get_stats(&submitter).unwrap();
        assert_eq!(stats.verified_submissions, 3);
    }

    #[test]
    fn test_waste_id_no_collision() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        env.mock_all_auths();

        let desc = String::from_str(&env, "Test");

        // Submit materials from different users
        let m1 = client.submit_material(&WasteType::Paper, &1000, &user1, &desc);
        let m2 = client.submit_material(&WasteType::Plastic, &2000, &user2, &desc);
        let m3 = client.submit_material(&WasteType::Metal, &3000, &user1, &desc);

        // Verify unique IDs
        assert_eq!(m1.id, 1);
        assert_eq!(m2.id, 2);
        assert_eq!(m3.id, 3);
        assert_ne!(m1.id, m2.id);
        assert_ne!(m2.id, m3.id);
    }

    #[test]
    fn test_waste_storage_efficiency() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        let desc = String::from_str(&env, "Efficiency test");

        // Submit material
        let material = client.submit_material(&WasteType::Paper, &5000, &user, &desc);

        // Retrieve should be efficient (single storage read)
        let retrieved = client.get_waste_by_id(&material.id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, material.id);
    }

    // Counter Storage System Tests
    #[test]
    fn test_waste_id_counter_initialization() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        let desc = String::from_str(&env, "First submission");
        
        // First submission should get ID 1
        let material = client.submit_material(&WasteType::Paper, &1000, &user, &desc);
        assert_eq!(material.id, 1);
    }

    #[test]
    fn test_waste_id_counter_increments_correctly() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        let desc = String::from_str(&env, "Test");

        // Submit multiple materials and verify sequential IDs
        let m1 = client.submit_material(&WasteType::Paper, &1000, &user, &desc);
        let m2 = client.submit_material(&WasteType::Plastic, &2000, &user, &desc);
        let m3 = client.submit_material(&WasteType::Metal, &3000, &user, &desc);
        let m4 = client.submit_material(&WasteType::Glass, &4000, &user, &desc);
        let m5 = client.submit_material(&WasteType::PetPlastic, &5000, &user, &desc);

        assert_eq!(m1.id, 1);
        assert_eq!(m2.id, 2);
        assert_eq!(m3.id, 3);
        assert_eq!(m4.id, 4);
        assert_eq!(m5.id, 5);
    }

    #[test]
    fn test_waste_id_no_reuse() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        let desc = String::from_str(&env, "Test");

        // Submit materials
        let m1 = client.submit_material(&WasteType::Paper, &1000, &user, &desc);
        let m2 = client.submit_material(&WasteType::Plastic, &2000, &user, &desc);
        
        // Even after retrieving, new submissions should get new IDs
        let _retrieved = client.get_material(&m1.id);
        let m3 = client.submit_material(&WasteType::Metal, &3000, &user, &desc);
        
        assert_eq!(m1.id, 1);
        assert_eq!(m2.id, 2);
        assert_eq!(m3.id, 3);
        
        // Verify no ID collision
        assert_ne!(m1.id, m2.id);
        assert_ne!(m2.id, m3.id);
        assert_ne!(m1.id, m3.id);
    }

    #[test]
    fn test_waste_id_counter_thread_safe_operations() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let user3 = Address::generate(&env);
        env.mock_all_auths();

        let desc = String::from_str(&env, "Concurrent test");

        // Simulate concurrent submissions from different users
        let m1 = client.submit_material(&WasteType::Paper, &1000, &user1, &desc);
        let m2 = client.submit_material(&WasteType::Plastic, &2000, &user2, &desc);
        let m3 = client.submit_material(&WasteType::Metal, &3000, &user3, &desc);
        let m4 = client.submit_material(&WasteType::Glass, &4000, &user1, &desc);

        // All IDs should be unique and sequential
        assert_eq!(m1.id, 1);
        assert_eq!(m2.id, 2);
        assert_eq!(m3.id, 3);
        assert_eq!(m4.id, 4);
    }

    #[test]
    fn test_waste_id_counter_with_batch_operations() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        // Submit single material first
        let desc = String::from_str(&env, "Single");
        let m1 = client.submit_material(&WasteType::Paper, &1000, &user, &desc);
        assert_eq!(m1.id, 1);

        // Submit batch
        let mut materials = soroban_sdk::Vec::new(&env);
        materials.push_back((
            WasteType::Plastic,
            2000u64,
            String::from_str(&env, "Batch 1"),
        ));
        materials.push_back((
            WasteType::Metal,
            3000u64,
            String::from_str(&env, "Batch 2"),
        ));

        let batch_results = client.submit_materials_batch(&materials, &user);
        
        // Batch should continue from where single left off
        assert_eq!(batch_results.get(0).unwrap().id, 2);
        assert_eq!(batch_results.get(1).unwrap().id, 3);

        // Submit another single material
        let m4 = client.submit_material(&WasteType::Glass, &4000, &user, &desc);
        assert_eq!(m4.id, 4);
    }

    #[test]
    fn test_waste_id_counter_persistence() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        let desc = String::from_str(&env, "Persistence test");

        // Submit materials
        client.submit_material(&WasteType::Paper, &1000, &user, &desc);
        client.submit_material(&WasteType::Plastic, &2000, &user, &desc);

        // Verify materials exist
        assert!(client.waste_exists(&1));
        assert!(client.waste_exists(&2));

        // Submit more materials - counter should persist
        let m3 = client.submit_material(&WasteType::Metal, &3000, &user, &desc);
        assert_eq!(m3.id, 3);
    }

    #[test]
    fn test_incentive_id_counter_initialization() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        
        // Test that incentive counter starts at 0
        let count = env.as_contract(&contract_id, || {
            ScavengerContract::get_incentive_count(&env)
        });
        assert_eq!(count, 0);
        
        // Test first increment
        let id1 = env.as_contract(&contract_id, || {
            ScavengerContract::next_incentive_id(&env)
        });
        assert_eq!(id1, 1);
        
        // Test second increment
        let id2 = env.as_contract(&contract_id, || {
            ScavengerContract::next_incentive_id(&env)
        });
        assert_eq!(id2, 2);
    }

    #[test]
    fn test_incentive_id_counter_increments_correctly() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        
        // Generate multiple IDs
        let id1 = env.as_contract(&contract_id, || {
            ScavengerContract::next_incentive_id(&env)
        });
        let id2 = env.as_contract(&contract_id, || {
            ScavengerContract::next_incentive_id(&env)
        });
        let id3 = env.as_contract(&contract_id, || {
            ScavengerContract::next_incentive_id(&env)
        });
        let id4 = env.as_contract(&contract_id, || {
            ScavengerContract::next_incentive_id(&env)
        });
        let id5 = env.as_contract(&contract_id, || {
            ScavengerContract::next_incentive_id(&env)
        });
        
        // Verify sequential increments
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
        assert_eq!(id4, 4);
        assert_eq!(id5, 5);
    }

    #[test]
    fn test_incentive_id_no_reuse() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        
        // Generate IDs
        let id1 = env.as_contract(&contract_id, || {
            ScavengerContract::next_incentive_id(&env)
        });
        let id2 = env.as_contract(&contract_id, || {
            ScavengerContract::next_incentive_id(&env)
        });
        let id3 = env.as_contract(&contract_id, || {
            ScavengerContract::next_incentive_id(&env)
        });
        
        // Verify all IDs are unique
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
        
        // Verify they are sequential (no gaps)
        assert_eq!(id2, id1 + 1);
        assert_eq!(id3, id2 + 1);
    }

    #[test]
    fn test_incentive_id_counter_persistence() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        
        // Generate some IDs
        env.as_contract(&contract_id, || {
            ScavengerContract::next_incentive_id(&env);
            ScavengerContract::next_incentive_id(&env);
        });
        
        // Check count persists
        let count = env.as_contract(&contract_id, || {
            ScavengerContract::get_incentive_count(&env)
        });
        assert_eq!(count, 2);
        
        // Generate more IDs
        let id3 = env.as_contract(&contract_id, || {
            ScavengerContract::next_incentive_id(&env)
        });
        assert_eq!(id3, 3);
        
        // Verify count updated
        let count = env.as_contract(&contract_id, || {
            ScavengerContract::get_incentive_count(&env)
        });
        assert_eq!(count, 3);
    }

    #[test]
    fn test_waste_and_incentive_counters_independent() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        env.mock_all_auths();

        let desc = String::from_str(&env, "Independence test");

        // Generate waste IDs
        let m1 = client.submit_material(&WasteType::Paper, &1000, &user, &desc);
        let m2 = client.submit_material(&WasteType::Plastic, &2000, &user, &desc);
        
        // Generate incentive IDs
        let i1 = env.as_contract(&contract_id, || {
            ScavengerContract::next_incentive_id(&env)
        });
        let i2 = env.as_contract(&contract_id, || {
            ScavengerContract::next_incentive_id(&env)
        });
        
        // Generate more waste IDs
        let m3 = client.submit_material(&WasteType::Metal, &3000, &user, &desc);
        
        // Generate more incentive IDs
        let i3 = env.as_contract(&contract_id, || {
            ScavengerContract::next_incentive_id(&env)
        });
        
        // Verify waste IDs are sequential
        assert_eq!(m1.id, 1);
        assert_eq!(m2.id, 2);
        assert_eq!(m3.id, 3);
        
        // Verify incentive IDs are sequential
        assert_eq!(i1, 1);
        assert_eq!(i2, 2);
        assert_eq!(i3, 3);
        
        // Verify counters are independent
        let waste_count = env.as_contract(&contract_id, || {
            ScavengerContract::get_waste_count(&env)
        });
        let incentive_count = env.as_contract(&contract_id, || {
            ScavengerContract::get_incentive_count(&env)
        });
        assert_eq!(waste_count, 3);
        assert_eq!(incentive_count, 3);
    }

    // Incentive-specific tests
    #[test]
    fn test_create_incentive() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let manufacturer = Address::generate(&env);
        env.mock_all_auths();

        // Register manufacturer
        client.register_participant(&manufacturer, &ParticipantRole::Manufacturer);

        // Create incentive
        let incentive = client.create_incentive(
            &WasteType::Plastic,
            &100,  // 100 tokens per kg
            &10000, // Max 10kg
            &manufacturer,
        );

        assert_eq!(incentive.id, 1);
        assert_eq!(incentive.waste_type, WasteType::Plastic);
        assert_eq!(incentive.reward, 100);
        assert_eq!(incentive.max_waste_amount, 10000);
        assert_eq!(incentive.rewarder, manufacturer);
        assert!(incentive.is_active);
        assert!(incentive.created_at > 0);
    }

    #[test]
    fn test_incentive_persistence() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let manufacturer = Address::generate(&env);
        env.mock_all_auths();

        client.register_participant(&manufacturer, &ParticipantRole::Manufacturer);

        let incentive = client.create_incentive(
            &WasteType::Metal,
            &200,
            &5000,
            &manufacturer,
        );

        // Retrieve and verify persistence
        let retrieved = client.get_incentive_by_id(&incentive.id);
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        
        assert_eq!(retrieved.id, incentive.id);
        assert_eq!(retrieved.waste_type, incentive.waste_type);
        assert_eq!(retrieved.reward, incentive.reward);
        assert_eq!(retrieved.max_waste_amount, incentive.max_waste_amount);
        assert_eq!(retrieved.rewarder, incentive.rewarder);
        assert_eq!(retrieved.is_active, incentive.is_active);
        assert_eq!(retrieved.created_at, incentive.created_at);
    }

    #[test]
    #[should_panic(expected = "Only manufacturers can create incentives")]
    fn test_create_incentive_non_manufacturer() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let collector = Address::generate(&env);
        env.mock_all_auths();

        // Register as collector (not manufacturer)
        client.register_participant(&collector, &ParticipantRole::Collector);

        // Should panic
        client.create_incentive(
            &WasteType::Paper,
            &50,
            &5000,
            &collector,
        );
    }

    #[test]
    #[should_panic(expected = "Reward must be greater than zero")]
    fn test_create_incentive_zero_reward() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let manufacturer = Address::generate(&env);
        env.mock_all_auths();

        client.register_participant(&manufacturer, &ParticipantRole::Manufacturer);

        // Should panic with zero reward
        client.create_incentive(
            &WasteType::Glass,
            &0,
            &5000,
            &manufacturer,
        );
    }

    #[test]
    #[should_panic(expected = "Max waste amount must be greater than zero")]
    fn test_create_incentive_zero_max_waste() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let manufacturer = Address::generate(&env);
        env.mock_all_auths();

        client.register_participant(&manufacturer, &ParticipantRole::Manufacturer);

        // Should panic with zero max waste amount
        client.create_incentive(
            &WasteType::PetPlastic,
            &100,
            &0,
            &manufacturer,
        );
    }

    #[test]
    fn test_update_incentive_status() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let manufacturer = Address::generate(&env);
        env.mock_all_auths();

        client.register_participant(&manufacturer, &ParticipantRole::Manufacturer);

        let incentive = client.create_incentive(
            &WasteType::Paper,
            &75,
            &8000,
            &manufacturer,
        );

        assert!(incentive.is_active);

        // Deactivate incentive
        let updated = client.update_incentive_status(&incentive.id, &false);
        assert!(!updated.is_active);

        // Reactivate incentive
        let updated = client.update_incentive_status(&incentive.id, &true);
        assert!(updated.is_active);
    }

    #[test]
    fn test_calculate_incentive_reward_basic() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let manufacturer = Address::generate(&env);
        env.mock_all_auths();

        client.register_participant(&manufacturer, &ParticipantRole::Manufacturer);

        // Create incentive: 100 tokens per kg, max 10kg
        let incentive = client.create_incentive(
            &WasteType::Plastic,
            &100,
            &10000,
            &manufacturer,
        );

        // Calculate reward for 5kg (5000g)
        let reward = client.calculate_incentive_reward(&incentive.id, &5000);
        // 5000g * 100 / 1000 = 500 tokens
        assert_eq!(reward, 500);
    }

    #[test]
    fn test_calculate_incentive_reward_capped() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let manufacturer = Address::generate(&env);
        env.mock_all_auths();

        client.register_participant(&manufacturer, &ParticipantRole::Manufacturer);

        // Create incentive: 100 tokens per kg, max 10kg
        let incentive = client.create_incentive(
            &WasteType::Metal,
            &100,
            &10000,
            &manufacturer,
        );

        // Calculate reward for 15kg (15000g) - should cap at 10kg
        let reward = client.calculate_incentive_reward(&incentive.id, &15000);
        // Capped at 10000g * 100 / 1000 = 1000 tokens
        assert_eq!(reward, 1000);
    }

    #[test]
    fn test_calculate_incentive_reward_inactive() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let manufacturer = Address::generate(&env);
        env.mock_all_auths();

        client.register_participant(&manufacturer, &ParticipantRole::Manufacturer);

        let incentive = client.create_incentive(
            &WasteType::Glass,
            &150,
            &5000,
            &manufacturer,
        );

        // Deactivate incentive
        client.update_incentive_status(&incentive.id, &false);

        // Calculate reward - should return 0 for inactive incentive
        let reward = client.calculate_incentive_reward(&incentive.id, &3000);
        assert_eq!(reward, 0);
    }

    #[test]
    fn test_calculate_incentive_reward_edge_cases() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let manufacturer = Address::generate(&env);
        env.mock_all_auths();

        client.register_participant(&manufacturer, &ParticipantRole::Manufacturer);

        let incentive = client.create_incentive(
            &WasteType::PetPlastic,
            &200,
            &10000,
            &manufacturer,
        );

        // Test with 0 waste
        let reward = client.calculate_incentive_reward(&incentive.id, &0);
        assert_eq!(reward, 0);

        // Test with exactly max amount
        let reward = client.calculate_incentive_reward(&incentive.id, &10000);
        assert_eq!(reward, 2000); // 10000 * 200 / 1000

        // Test with 1 gram
        let reward = client.calculate_incentive_reward(&incentive.id, &1);
        assert_eq!(reward, 0); // 1 * 200 / 1000 = 0 (integer division)
    }

    #[test]
    fn test_incentive_exists() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let manufacturer = Address::generate(&env);
        env.mock_all_auths();

        client.register_participant(&manufacturer, &ParticipantRole::Manufacturer);

        // Check non-existent incentive
        assert!(!client.incentive_exists(&1));

        // Create incentive
        let incentive = client.create_incentive(
            &WasteType::Paper,
            &50,
            &5000,
            &manufacturer,
        );

        // Check existing incentive
        assert!(client.incentive_exists(&incentive.id));
        assert!(!client.incentive_exists(&999));
    }

    #[test]
    fn test_get_incentives_by_waste_type() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let manufacturer = Address::generate(&env);
        env.mock_all_auths();

        client.register_participant(&manufacturer, &ParticipantRole::Manufacturer);

        // Create multiple incentives
        client.create_incentive(&WasteType::Plastic, &100, &10000, &manufacturer);
        client.create_incentive(&WasteType::Metal, &200, &5000, &manufacturer);
        client.create_incentive(&WasteType::Plastic, &150, &8000, &manufacturer);
        client.create_incentive(&WasteType::Paper, &75, &12000, &manufacturer);

        // Get plastic incentives
        let plastic_incentives = client.get_incentives_by_waste_type(&WasteType::Plastic);
        assert_eq!(plastic_incentives.len(), 2);

        // Get metal incentives
        let metal_incentives = client.get_incentives_by_waste_type(&WasteType::Metal);
        assert_eq!(metal_incentives.len(), 1);

        // Get glass incentives (none)
        let glass_incentives = client.get_incentives_by_waste_type(&WasteType::Glass);
        assert_eq!(glass_incentives.len(), 0);
    }

    #[test]
    fn test_get_active_incentives() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let manufacturer = Address::generate(&env);
        env.mock_all_auths();

        client.register_participant(&manufacturer, &ParticipantRole::Manufacturer);

        // Create multiple incentives
        let i1 = client.create_incentive(&WasteType::Plastic, &100, &10000, &manufacturer);
        let i2 = client.create_incentive(&WasteType::Metal, &200, &5000, &manufacturer);
        let i3 = client.create_incentive(&WasteType::Paper, &75, &8000, &manufacturer);

        // All should be active initially
        let active = client.get_active_incentives();
        assert_eq!(active.len(), 3);

        // Deactivate one
        client.update_incentive_status(&i2.id, &false);

        // Should have 2 active
        let active = client.get_active_incentives();
        assert_eq!(active.len(), 2);

        // Deactivate another
        client.update_incentive_status(&i1.id, &false);

        // Should have 1 active
        let active = client.get_active_incentives();
        assert_eq!(active.len(), 1);
        assert_eq!(active.get(0).unwrap().id, i3.id);
    }

    #[test]
    fn test_multiple_manufacturers_incentives() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let manufacturer1 = Address::generate(&env);
        let manufacturer2 = Address::generate(&env);
        env.mock_all_auths();

        client.register_participant(&manufacturer1, &ParticipantRole::Manufacturer);
        client.register_participant(&manufacturer2, &ParticipantRole::Manufacturer);

        // Each manufacturer creates incentives
        let i1 = client.create_incentive(&WasteType::Plastic, &100, &10000, &manufacturer1);
        let i2 = client.create_incentive(&WasteType::Plastic, &120, &8000, &manufacturer2);

        // Verify both exist and are independent
        let retrieved1 = client.get_incentive_by_id(&i1.id).unwrap();
        let retrieved2 = client.get_incentive_by_id(&i2.id).unwrap();

        assert_eq!(retrieved1.rewarder, manufacturer1);
        assert_eq!(retrieved2.rewarder, manufacturer2);
        assert_ne!(retrieved1.reward, retrieved2.reward);
    }

    #[test]
    fn test_incentive_storage_deterministic() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let manufacturer = Address::generate(&env);
        env.mock_all_auths();

        client.register_participant(&manufacturer, &ParticipantRole::Manufacturer);

        // Create incentive
        let incentive = client.create_incentive(
            &WasteType::Metal,
            &250,
            &15000,
            &manufacturer,
        );

        // Retrieve multiple times - should be consistent
        let r1 = client.get_incentive_by_id(&incentive.id).unwrap();
        let r2 = client.get_incentive_by_id(&incentive.id).unwrap();
        let r3 = client.get_incentive_by_id(&incentive.id).unwrap();

        assert_eq!(r1, r2);
        assert_eq!(r2, r3);
    }

    #[test]
    fn test_incentive_all_waste_types() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let manufacturer = Address::generate(&env);
        env.mock_all_auths();

        client.register_participant(&manufacturer, &ParticipantRole::Manufacturer);

        // Create incentive for each waste type
        let i1 = client.create_incentive(&WasteType::Paper, &50, &10000, &manufacturer);
        let i2 = client.create_incentive(&WasteType::PetPlastic, &100, &8000, &manufacturer);
        let i3 = client.create_incentive(&WasteType::Plastic, &90, &9000, &manufacturer);
        let i4 = client.create_incentive(&WasteType::Metal, &200, &5000, &manufacturer);
        let i5 = client.create_incentive(&WasteType::Glass, &75, &12000, &manufacturer);

        // Verify all were created correctly
        assert_eq!(i1.waste_type, WasteType::Paper);
        assert_eq!(i2.waste_type, WasteType::PetPlastic);
        assert_eq!(i3.waste_type, WasteType::Plastic);
        assert_eq!(i4.waste_type, WasteType::Metal);
        assert_eq!(i5.waste_type, WasteType::Glass);
    }

    #[test]
    fn test_incentive_reward_calculation_no_overflow() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ScavengerContract);
        let client = ScavengerContractClient::new(&env, &contract_id);

        let manufacturer = Address::generate(&env);
        env.mock_all_auths();

        client.register_participant(&manufacturer, &ParticipantRole::Manufacturer);

        // Create incentive with large values
        let incentive = client.create_incentive(
            &WasteType::Metal,
            &1000,
            &1000000, // 1000kg max
            &manufacturer,
        );

        // Calculate reward for large amount
        let reward = client.calculate_incentive_reward(&incentive.id, &500000);
        // 500000 * 1000 / 1000 = 500000 tokens
        assert_eq!(reward, 500000);
    }
}

