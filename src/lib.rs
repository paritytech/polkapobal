#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod polkapobal {
    use ink::{env::debug_println, prelude::string::String, prelude::vec::Vec, storage::Mapping};

    #[ink(event)]
    pub struct SelectionEraChanged {
        new_era: u32,
    }

    #[ink(event)]
    pub struct MemberRegistered {
        /// The member that was added.
        #[ink(topic)]
        member: AccountId,
    }

    #[ink(event)]
    pub struct MemberDeregistered {
        /// The member that was removed.
        #[ink(topic)]
        member: AccountId,
    }

    #[ink(event)]
    pub struct MembersCleared {}

    #[ink(event)]
    pub struct TaskAdded {
        /// The task that was added.
        #[ink(topic)]
        task: String,
    }

    #[ink(event)]
    pub struct TaskRemoved {
        /// The task that was removed.
        #[ink(topic)]
        task: String,
    }

    #[ink(event)]
    pub struct TasksCleared {}

    #[ink(event)]
    pub struct TaskFunded {
        #[ink(topic)]
        task: String,
        #[ink(topic)]
        donor: AccountId,
        amount: Balance,
    }

    #[ink(event)]
    pub struct NewEraStarted {
        era: u32,
        #[ink(topic)]
        participants: Vec<AccountId>,
        #[ink(topic)]
        task: String,
    }

    #[ink(storage)]
    pub struct Polkapobal {
        owner: AccountId,
        members: Vec<AccountId>,
        is_member: Mapping<AccountId, ()>,
        tasks: Vec<String>,
        // task -> (is_complete, funds)
        task_info: Mapping<String, (bool, Balance)>,
        unclaimed_funds: Balance,
        start_block: u32,
        // How many blocks until next selection
        next_selection: u32,
        // Last selection block number
        last_selection: u32,
        active_participants: Vec<AccountId>,
        // (task, is_complete)
        active_task: Option<(String, bool)>,
        // task -> proof hash
        proofs: Mapping<String, Hash>,
    }

    impl Polkapobal {
        #[ink(constructor)]
        pub fn new(selection_era: u32) -> Self {
            let current_block = Self::env().block_number();
            Polkapobal {
                owner: Self::env().caller(),
                members: Vec::new(),
                is_member: Mapping::default(),
                tasks: Vec::new(),
                task_info: Mapping::default(),
                unclaimed_funds: 0,
                start_block: current_block,
                next_selection: selection_era,
                last_selection: current_block,
                active_participants: Vec::new(),
                active_task: None,
                proofs: Mapping::default(),
            }
        }

        #[ink(message)]
        pub fn set_selection_era(&mut self, selection_era: u32) {
            self.ensure_owner();

            self.next_selection = selection_era;

            self.env().emit_event(SelectionEraChanged {
                new_era: selection_era,
            })
        }

        #[ink(message)]
        pub fn register_member(&mut self) {
            let caller = self.env().caller();

            // Ensure that the member does not exist
            assert!(!self.is_member.contains(&caller), "Member already exists");

            self.is_member.insert(caller, &());
            self.members.push(caller);

            self.env().emit_event(MemberRegistered { member: caller });
        }

        #[ink(message)]
        pub fn deregister_member(&mut self) {
            let caller = self.env().caller();

            // Ensure that the member exists
            self.ensure_member();

            // Search for index of member
            let index = self
                .members
                .iter()
                .position(|x| *x == caller)
                .expect("Member existence verified before calling");
            self.members.swap_remove(index);
            self.is_member.remove(caller);

            self.env().emit_event(MemberDeregistered { member: caller });
        }

        #[ink(message)]
        pub fn clear_members(&mut self) {
            self.ensure_owner();

            // Iterate over `members` vec and remove each member.
            // Done in reverse so the member indices' do not change.
            for (i, member) in self.members.clone().iter().enumerate().rev() {
                self.is_member.remove(member);
                self.members.swap_remove(i);
            }

            self.env().emit_event(MembersCleared {});
        }

        #[ink(message)]
        pub fn add_task(&mut self, task: String) {
            self.ensure_member();

            // Ensure that the task does not exist
            assert!(!self.task_info.contains(&task), "Task already exists");

            self.task_info.insert(&task, &(false, 0));
            self.tasks.push(task.clone());

            self.env().emit_event(TaskAdded { task: task });
        }

        #[ink(message)]
        pub fn remove_task(&mut self, task: String) {
            self.ensure_owner();

            // Ensure that the task does exists
            assert!(self.task_info.contains(&task), "Task does not exist");

            let task_info = self
                .task_info
                .take(&task)
                .expect("Task existence verified before calling");

            // If task is funded, add funds to unclaimed funds
            self.unclaimed_funds = self
                .unclaimed_funds
                .checked_add(task_info.1)
                .expect("Balance overflow");

            // Search for index of member
            let index = self
                .tasks
                .iter()
                .position(|x| *x == task)
                .expect("Task existence verified before calling");
            self.tasks.swap_remove(index);

            self.env().emit_event(TaskRemoved { task: task });
        }

        #[ink(message)]
        pub fn clear_tasks(&mut self) {
            self.ensure_owner();

            // Iterate over `active_tasks` vec and remove each member.
            // Done in reverse so the task indices' do not change.
            for (i, task) in self.tasks.clone().iter().enumerate().rev() {
                let task_info = self
                    .task_info
                    .take(task)
                    .expect("Task existence verified before calling");
                self.unclaimed_funds = self
                    .unclaimed_funds
                    .checked_add(task_info.1)
                    .expect("Balance overflow");

                self.tasks.swap_remove(i);
            }

            self.env().emit_event(TasksCleared {});
        }

        #[ink(message, payable)]
        pub fn fund_task(&mut self, task: String) {
            let caller = self.env().caller();
            let transferred = self.env().transferred_value();

            //Ensure that the task does exist
            assert!(self.task_info.contains(&task), "Task does not exist");

            let mut task_info = self
                .task_info
                .get(&task)
                .expect("Task existence verified before calling");

            assert!(task_info.0 == false, "Task already completed");

            task_info.1 = task_info
                .1
                .checked_add(transferred)
                .expect("Balance overflow");

            self.task_info.insert(&task, &task_info);

            self.env().emit_event(TaskFunded {
                task: task,
                donor: caller,
                amount: transferred,
            });
        }

        #[ink(message)]
        pub fn start_new_era(&mut self) {
            self.ensure_era_reached();
            // TODO: simplified logic for MVP
            self.ensure_active_task_complete();

            let members = self.randomly_select_members();
            let task = self.randomly_select_task();

            self.last_selection = self.env().block_number();
            self.active_participants = members.clone();
            self.active_task = Some((task.clone(), false));

            self.env().emit_event(NewEraStarted {
                era: Self::env().block_number(),
                participants: members,
                task: task,
            });
        }

        #[ink(message)]
        pub fn upload_completion_proof(&mut self, proof: Hash) {
            let caller = self.env().caller();

            assert!(
                self.active_participants.contains(&caller),
                "Caller must be active participant"
            );

            let active_task = self.active_task.as_ref().expect("Active task must exist");
            self.proofs.insert(&active_task.0, &proof);
        }

        #[ink(message)]
        pub fn complete_task(&mut self) {
            self.ensure_owner();

            if let Some(task) = self.active_task.clone() {
                self.active_task = Some((task.0.clone(), true));
                let task_info = self.task_info.get(&task.0).expect("Task must exist");

                self.disburse_rewards(self.active_participants.clone(), task_info.1);
                // set reward balance to 0
                self.task_info.insert(&task.0, &(true, 0));
            }
        }

        fn randomly_select_members(&self) -> Vec<AccountId> {
            // TODO: use randomness when chain extension is added

            self.ensure_non_empty_members();

            let mut members: Vec<AccountId> = Vec::new();
            for i in 0..4 {
                let member =
                    self.members[(Self::env().block_number() as usize + i) % self.members.len()];
                members.push(member);
            }

            members
        }

        fn randomly_select_task(&self) -> String {
            self.ensure_non_empty_tasks();

            // TODO: use randomness
            self.tasks[Self::env().block_number() as usize % self.tasks.len()].clone()
        }

        fn disburse_rewards(&mut self, beneficiaries: Vec<AccountId>, amount: Balance) {
            assert!(
                Self::env().balance() >= amount,
                "Contract has insufficient funds"
            );

            let amount_per_beneficiary = amount / beneficiaries.len() as u128;
            let remainder = amount % beneficiaries.len() as u128;

            beneficiaries.iter().for_each(|beneficiary| {
                if self
                    .env()
                    .transfer(*beneficiary, amount_per_beneficiary)
                    .is_err()
                {
                    // add failed transfer to unclaimed funds
                    self.unclaimed_funds = self
                        .unclaimed_funds
                        .checked_add(amount_per_beneficiary)
                        .expect("Balance overflow");
                }
            });

            // Add the indivisable amount (remainder) to unclaimed funds
            self.unclaimed_funds = self
                .unclaimed_funds
                .checked_add(remainder)
                .expect("Balance overflow");
        }

        fn ensure_owner(&self) {
            assert_eq!(self.env().caller(), self.owner, "Only owner can call");
        }

        fn ensure_member(&self) {
            assert!(
                self.is_member.contains(&self.env().caller()),
                "Must be a member to call"
            );
        }

        fn ensure_non_empty_members(&self) {
            assert!(self.members.len() > 0, "Must have at least one member");
        }

        fn ensure_non_empty_tasks(&self) {
            assert!(self.tasks.len() > 0, "Must have at least one task");
        }

        fn ensure_active_task_complete(&self) {
            // TODO: benchmark against using self.tasks mapping
            if let Some(task) = &self.active_task {
                assert!(task.1, "Active task must be completed");
            }
            // if None, simply return
        }

        fn ensure_era_reached(&self) {
            assert!(
                self.env().block_number() >= self.last_selection + self.next_selection,
                "Selection era not reached"
            );
        }
    }

    #[cfg(test)]
    mod tests {
        /// Imports all the definitions from the outer scope so we can use them here.
        use super::*;
        use ink::codegen::Env;
        use ink::env::test::{self};

        const DEFAULT_SELECTION_ERA: u32 = 10;

        fn create_default_contract() -> Polkapobal {
            Polkapobal::new(DEFAULT_SELECTION_ERA)
        }

        fn set_balance(account_id: AccountId, balance: Balance) {
            ink::env::test::set_account_balance::<ink::env::DefaultEnvironment>(account_id, balance)
        }

        fn get_balance(account_id: AccountId) -> Balance {
            ink::env::test::get_account_balance::<ink::env::DefaultEnvironment>(account_id)
                .expect("Cannot get account balance")
        }

        fn advance_block(num_blocks: u32) {
            for _ in 0..num_blocks {
                ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            }
        }

        #[ink::test]
        fn construction_works() {
            let init_block = 5;
            // non-zero block number to test
            advance_block(init_block);

            let expected = Polkapobal {
                owner: AccountId::from([0x01; 32]),
                members: Vec::new(),
                is_member: Mapping::default(),
                tasks: Vec::new(),
                task_info: Mapping::default(),
                unclaimed_funds: 0,
                start_block: init_block,
                next_selection: DEFAULT_SELECTION_ERA,
                last_selection: init_block,
                active_participants: Vec::new(),
                active_task: None,
                proofs: Mapping::default(),
            };

            let contract = Polkapobal::new(DEFAULT_SELECTION_ERA);
            assert_eq!(contract.owner, expected.owner);
            assert_eq!(contract.members.len(), 0);
            assert_eq!(contract.tasks.len(), 0);
            assert_eq!(contract.unclaimed_funds, expected.unclaimed_funds);
            assert_eq!(contract.start_block, expected.start_block);
            assert_eq!(contract.next_selection, expected.next_selection);
            assert_eq!(contract.last_selection, expected.last_selection);
            assert_eq!(contract.active_participants.len(), 0);
            assert_eq!(contract.active_task, None);
        }

        #[ink::test]
        fn set_selection_era_works() {
            let mut contract = create_default_contract();

            contract.set_selection_era(20);

            assert_eq!(contract.next_selection, 20);
            assert_eq!(test::recorded_events().count(), 1);
        }

        #[ink::test]
        fn register_member_works() {
            let mut contract = create_default_contract();

            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

            contract.register_member();

            ink::env::test::set_caller::<Environment>(accounts.bob);
            contract.register_member();

            assert_eq!(contract.members.len(), 2);
            assert!(contract.members.contains(&accounts.alice));
            assert!(contract.members.contains(&accounts.bob));
            assert!(contract.is_member.contains(accounts.alice));
            assert!(contract.is_member.contains(accounts.bob));
            assert_eq!(test::recorded_events().count(), 2);
        }

        #[ink::test]
        fn deregister_member_works() {
            let mut contract = create_default_contract();

            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

            contract.register_member();

            ink::env::test::set_caller::<Environment>(accounts.bob);
            contract.register_member();

            ink::env::test::set_caller::<Environment>(accounts.charlie);
            contract.register_member();

            assert_eq!(contract.members.len(), 3);
            contract.deregister_member();
            assert_eq!(contract.members.len(), 2);
            assert!(!contract.members.contains(&accounts.charlie));
            assert!(!contract.is_member.contains(&accounts.charlie));

            ink::env::test::set_caller::<Environment>(accounts.bob);
            contract.deregister_member();
            assert_eq!(contract.members.len(), 1);
            assert!(!contract.members.contains(&accounts.bob));
            assert!(!contract.is_member.contains(&accounts.bob));

            ink::env::test::set_caller::<Environment>(accounts.alice);
            contract.deregister_member();
            assert_eq!(contract.members.len(), 0);
            assert!(!contract.members.contains(&accounts.alice));
            assert!(!contract.is_member.contains(&accounts.alice));
            assert_eq!(test::recorded_events().count(), 6);
        }

        #[ink::test]
        fn clear_members_works() {
            let mut contract = create_default_contract();

            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

            contract.register_member();

            ink::env::test::set_caller::<Environment>(accounts.bob);
            contract.register_member();

            ink::env::test::set_caller::<Environment>(accounts.charlie);
            contract.register_member();

            assert_eq!(contract.members.len(), 3);

            ink::env::test::set_caller::<Environment>(accounts.alice);
            contract.clear_members();

            assert_eq!(contract.members.len(), 0);
            assert!(!contract.is_member.contains(&accounts.alice));
            assert!(!contract.is_member.contains(&accounts.bob));
            assert!(!contract.is_member.contains(&accounts.charlie));
            assert_eq!(test::recorded_events().count(), 4);
        }

        #[ink::test]
        fn add_task_works() {
            let mut contract = create_default_contract();

            contract.register_member();

            let task1 = String::from("Task 1");
            let task2 = String::from("Task 2");
            contract.add_task(task1.clone());
            contract.add_task(task2.clone());

            assert_eq!(contract.tasks.len(), 2);
            assert_eq!(contract.task_info.get(&task1).unwrap(), (false, 0));
            assert_eq!(contract.task_info.get(&task2).unwrap(), (false, 0));
            assert_eq!(test::recorded_events().count(), 3);
        }

        #[ink::test]
        fn remove_task_works() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            ink::env::test::set_caller::<Environment>(accounts.eve);

            let mut contract = create_default_contract();
            let contract_address = contract.env().account_id();

            contract.register_member();

            let task1 = String::from("Task 1");
            let task2 = String::from("Task 2");
            contract.add_task(task1.clone());
            contract.add_task(task2.clone());

            assert_eq!(contract.tasks.len(), 2);
            assert_eq!(contract.task_info.get(&task1).unwrap(), (false, 0));
            assert_eq!(contract.task_info.get(&task2).unwrap(), (false, 0));

            set_balance(accounts.eve, 100);
            set_balance(contract_address, 0);

            ink::env::pay_with_call!(contract.fund_task(task1.clone()), 10);
            ink::env::pay_with_call!(contract.fund_task(task2.clone()), 20);

            contract.remove_task(task1.clone());

            assert_eq!(contract.tasks.len(), 1);
            assert_eq!(contract.task_info.get(&task1), None);
            assert_eq!(contract.unclaimed_funds, 10);
            assert_eq!(get_balance(contract_address), 30);

            contract.remove_task(task2.clone());

            assert_eq!(contract.tasks.len(), 0);
            assert_eq!(contract.task_info.get(&task2), None);
            assert_eq!(contract.unclaimed_funds, 30);
            assert_eq!(get_balance(contract_address), 30);

            assert_eq!(test::recorded_events().count(), 7);
        }

        #[ink::test]
        fn clear_tasks_works() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            ink::env::test::set_caller::<Environment>(accounts.eve);

            let mut contract = create_default_contract();
            let contract_address = contract.env().account_id();

            contract.register_member();

            let task1 = String::from("Task 1");
            let task2 = String::from("Task 2");
            let task3 = String::from("Task 3");
            contract.add_task(task1.clone());
            contract.add_task(task2.clone());
            contract.add_task(task3.clone());

            set_balance(accounts.eve, 100);
            set_balance(contract_address, 0);

            ink::env::pay_with_call!(contract.fund_task(task1.clone()), 10);
            ink::env::pay_with_call!(contract.fund_task(task2.clone()), 20);

            assert_eq!(contract.tasks.len(), 3);

            contract.clear_tasks();

            assert_eq!(contract.tasks.len(), 0);

            assert!(!contract.task_info.contains(&task1));
            assert!(!contract.task_info.contains(&task2));
            assert!(!contract.task_info.contains(&task3));
            assert_eq!(contract.unclaimed_funds, 30);
            assert_eq!(get_balance(contract_address), 30);

            assert_eq!(test::recorded_events().count(), 7);
        }

        #[ink::test]
        fn fund_task_works() {
            let mut contract = create_default_contract();

            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

            let contract_address = contract.env().account_id();

            ink::env::test::set_caller::<Environment>(accounts.eve);

            contract.register_member();

            let task1 = String::from("Task 1");
            contract.add_task(task1.clone());

            assert_eq!(contract.tasks.len(), 1);
            assert_eq!(contract.task_info.get(&task1).unwrap(), (false, 0));

            set_balance(accounts.eve, 100);
            set_balance(contract_address, 0);

            ink::env::pay_with_call!(contract.fund_task(task1.clone()), 10);

            assert_eq!(get_balance(contract_address), 10);
            assert_eq!(get_balance(accounts.eve), 100 - 10);
            assert_eq!(contract.task_info.get(&task1).unwrap(), (false, 10));

            assert_eq!(test::recorded_events().count(), 3);
        }

        #[ink::test]
        fn start_new_era_works() {
            let init_block = 10;
            advance_block(init_block);

            let mut contract = create_default_contract();
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

            let mut members: Vec<AccountId> = Vec::new();

            let num_members: u8 = 100;
            // register 100 members with account ids 0..99
            for i in 0..num_members {
                let member = AccountId::from([i; 32]);
                members.push(member);

                ink::env::test::set_caller::<Environment>(member);
                contract.register_member();
            }

            ink::env::test::set_caller::<Environment>(accounts.alice);

            let mut tasks: Vec<String> = Vec::new();

            let num_tasks: u8 = 100;
            // create 100 tasks
            for i in 0..num_tasks {
                let task = String::from(format!("Task {}", i));
                tasks.push(task.clone());

                contract.add_task(task);
            }

            assert_eq!(contract.members.len(), num_members as usize);
            assert_eq!(contract.members, members);
            assert_eq!(contract.last_selection, init_block);

            // advance block to selection era
            advance_block(DEFAULT_SELECTION_ERA);

            contract.start_new_era();

            assert_eq!(
                contract.last_selection,
                init_block + contract.next_selection
            );
            assert_eq!(contract.active_participants.len(), 4);
            assert_eq!(contract.active_task.is_some(), true);

            // TODO: add distribution tests when randomness is added
        }

        // TODO: unit tests for:
        // - upload_completion_proof
        // - complete_task
        // - start_new_era passes and panics when task complete and not complete, respectively

        #[ink::test]
        fn upload_completion_proof_works() {
            let mut contract = create_default_contract();
            advance_block(DEFAULT_SELECTION_ERA);

            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

            let task = String::from("Task");
            let proof = Hash::from([0x01; 32]);

            let mut members = Vec::new();
            let num_members: u8 = 4;
            for i in 0..num_members {
                let member = AccountId::from([i; 32]);
                members.push(member);

                ink::env::test::set_caller::<Environment>(member);
                contract.register_member();
            }

            contract.add_task(task.clone());
            contract.start_new_era();

            ink::env::test::set_caller::<Environment>(accounts.alice);
            contract.upload_completion_proof(proof);

            assert_eq!(contract.proofs.get(&task).unwrap(), proof);
        }

        #[ink::test]
        fn disburse_rewards_works() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

            let mut contract = create_default_contract();

            let contract_address = contract.env().account_id();

            let beneficiaries = vec![
                accounts.bob,
                accounts.charlie,
                accounts.django,
                accounts.eve,
            ];

            set_balance(contract_address, 1000);

            // sanity check, set balances to 0
            for beneficiary in beneficiaries.clone() {
                set_balance(beneficiary, 0);
            }

            contract.disburse_rewards(beneficiaries.clone(), 100);

            // closure to assert the balances of the beneficiaries
            let assert_balances = |be: &Vec<AccountId>, expected_balance| {
                for beneficiary in be {
                    debug_println!("Beneficiary: {:?}", beneficiary);
                    assert_eq!(get_balance(*beneficiary), expected_balance);
                }
            };

            assert_eq!(get_balance(contract_address), 900);
            assert_balances(&beneficiaries, 25);

            // indivisable by 4
            let indivisable_amount: Balance = 75;
            let beneficiary_amount = indivisable_amount / 4;
            let remainder = indivisable_amount % 4;

            // sanity check, ensure amount is truncated to 18
            assert_eq!(beneficiary_amount, 18);

            contract.disburse_rewards(beneficiaries.clone(), indivisable_amount);

            // remainder is 3, so only 72 is disbursed
            assert_eq!(
                get_balance(contract_address),
                900 - (indivisable_amount - remainder)
            );
            assert_balances(&beneficiaries, 25 + beneficiary_amount);
            assert_eq!(contract.unclaimed_funds, remainder);
        }

        #[ink::test]
        #[should_panic(expected = "Only owner can call")]
        fn set_selection_era_panics() {
            let mut contract = create_default_contract();
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            ink::env::test::set_caller::<Environment>(accounts.eve);

            contract.set_selection_era(20);
        }

        #[ink::test]
        #[should_panic(expected = "Member already exists")]
        fn register_member_panics() {
            let mut contract = create_default_contract();

            contract.register_member();
            // Should panic here
            contract.register_member();
        }

        #[ink::test]
        #[should_panic(expected = "Must be a member to call")]
        fn deregister_member_panics() {
            let mut contract = create_default_contract();

            contract.deregister_member();
        }

        #[ink::test]
        #[should_panic(expected = "Only owner can call")]
        fn clear_members_panics() {
            let mut contract = create_default_contract();

            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

            ink::env::test::set_caller::<Environment>(accounts.bob);
            contract.clear_members();
        }

        #[ink::test]
        #[should_panic(expected = "Task already exists")]
        fn add_task_twice_panics() {
            let mut contract = create_default_contract();

            contract.register_member();

            let task = String::from("Task");
            contract.add_task(task.clone());
            contract.add_task(task);
        }

        #[ink::test]
        #[should_panic(expected = "Must be a member to call")]
        fn add_task_when_not_member_panics() {
            let mut contract = create_default_contract();

            let task = String::from("Task");
            contract.add_task(task.clone());
        }

        #[ink::test]
        #[should_panic(expected = "Task does not exist")]
        fn remove_nonexistent_task_panics() {
            let mut contract = create_default_contract();

            contract.register_member();

            let task = String::from("Task");
            // task does not exist
            contract.remove_task(task);
        }

        #[ink::test]
        #[should_panic(expected = "Only owner can call")]
        fn remove_task_when_not_member_panics() {
            let mut contract = create_default_contract();

            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

            contract.register_member();

            let task = String::from("Task");
            contract.add_task(task.clone());

            ink::env::test::set_caller::<Environment>(accounts.bob);
            contract.remove_task(task);
        }

        #[ink::test]
        #[should_panic(expected = "Only owner can call")]
        fn clear_tasks_panic() {
            let mut contract = create_default_contract();

            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

            ink::env::test::set_caller::<Environment>(accounts.eve);

            contract.register_member();

            let task = String::from("Task");
            contract.add_task(task.clone());

            contract.clear_tasks();
        }

        //TODO: test for panic if task is already completed

        #[ink::test]
        #[should_panic(expected = "Task does not exist")]
        fn fund_nonexistent_task_panics() {
            let mut contract = create_default_contract();

            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

            ink::env::test::set_caller::<Environment>(accounts.eve);

            contract.register_member();

            set_balance(accounts.eve, 100);

            let task = String::from("Task");
            // task does not exist
            ink::env::pay_with_call!(contract.fund_task(task), 10);
        }

        #[ink::test]
        #[should_panic(expected = "Selection era not reached")]
        fn start_new_era_when_era_not_reached_panics() {
            advance_block(20);

            let mut contract = create_default_contract();

            contract.start_new_era();
        }

        #[ink::test]
        #[should_panic(expected = "Must have at least one member")]
        fn start_new_era_with_empty_members_panics() {
            let mut contract = create_default_contract();

            advance_block(DEFAULT_SELECTION_ERA);

            contract.start_new_era();
        }

        #[ink::test]
        #[should_panic(expected = "Must have at least one task")]
        fn start_new_era_with_empty_tasks_panics() {
            let mut contract = create_default_contract();
            contract.register_member();

            advance_block(DEFAULT_SELECTION_ERA);

            contract.start_new_era();
        }

        #[ink::test]
        #[should_panic(expected = "Caller must be active participant")]
        fn upload_completion_proof_not_participant_panics() {
            let mut contract = create_default_contract();

            let task = String::from("Task");
            let proof = Hash::from([0x01; 32]);

            contract.register_member();
            contract.add_task(task.clone());

            contract.upload_completion_proof(proof);
        }
    }
}
