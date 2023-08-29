#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod polkapobal {
    use ink::{prelude::string::String, prelude::vec::Vec, storage::Mapping};

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

    #[ink(storage)]
    pub struct Polkapobal {
        owner: AccountId,
        members: Vec<AccountId>,
        is_member: Mapping<AccountId, ()>,
        active_tasks: Vec<String>,
        // task -> (is_complete, funds)
        task_info: Mapping<String, (bool, Balance)>,
        unclaimed_funds: Balance,
    }

    impl Polkapobal {
        #[ink(constructor)]
        pub fn new() -> Self {
            Polkapobal {
                owner: Self::env().caller(),
                members: Vec::new(),
                is_member: Mapping::default(),
                active_tasks: Vec::new(),
                task_info: Mapping::default(),
                unclaimed_funds: 0,
            }
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
            self.active_tasks.push(task.clone());

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
                .active_tasks
                .iter()
                .position(|x| *x == task)
                .expect("Task existence verified before calling");
            self.active_tasks.swap_remove(index);

            self.env().emit_event(TaskRemoved { task: task });
        }

        #[ink(message)]
        pub fn clear_tasks(&mut self) {
            self.ensure_owner();

            // Iterate over `active_tasks` vec and remove each member.
            // Done in reverse so the task indices' do not change.
            for (i, task) in self.active_tasks.clone().iter().enumerate().rev() {
                let task_info = self
                    .task_info
                    .take(task)
                    .expect("Task existence verified before calling");
                self.unclaimed_funds = self
                    .unclaimed_funds
                    .checked_add(task_info.1)
                    .expect("Balance overflow");

                self.active_tasks.swap_remove(i);
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

        fn ensure_owner(&self) {
            assert_eq!(self.env().caller(), self.owner, "Only owner can call");
        }

        fn ensure_member(&self) {
            assert!(
                self.is_member.contains(&self.env().caller()),
                "Must be a member to call"
            );
        }
    }

    #[cfg(test)]
    mod tests {
        /// Imports all the definitions from the outer scope so we can use them here.
        use super::*;
        use ink::codegen::Env;
        use ink::env::test;

        fn set_balance(account_id: AccountId, balance: Balance) {
            ink::env::test::set_account_balance::<ink::env::DefaultEnvironment>(account_id, balance)
        }

        fn get_balance(account_id: AccountId) -> Balance {
            ink::env::test::get_account_balance::<ink::env::DefaultEnvironment>(account_id)
                .expect("Cannot get account balance")
        }

        #[ink::test]
        fn construction_works() {
            let expected = Polkapobal {
                owner: AccountId::from([0x01; 32]),
                members: Vec::new(),
                is_member: Mapping::default(),
                active_tasks: Vec::new(),
                task_info: Mapping::default(),
                unclaimed_funds: 0,
            };

            let contract = Polkapobal::new();
            assert_eq!(contract.owner, expected.owner);
            assert_eq!(contract.members.len(), 0);
            assert_eq!(contract.active_tasks.len(), 0);
        }

        #[ink::test]
        fn register_member_works() {
            let mut contract = Polkapobal::new();

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
            let mut contract = Polkapobal::new();

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
            let mut contract = Polkapobal::new();

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
            let mut contract = Polkapobal::new();

            contract.register_member();

            let task1 = String::from("Task 1");
            let task2 = String::from("Task 2");
            contract.add_task(task1.clone());
            contract.add_task(task2.clone());

            assert_eq!(contract.active_tasks.len(), 2);
            assert_eq!(contract.task_info.get(&task1).unwrap(), (false, 0));
            assert_eq!(contract.task_info.get(&task2).unwrap(), (false, 0));
            assert_eq!(test::recorded_events().count(), 3);
        }

        #[ink::test]
        fn remove_task_works() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            ink::env::test::set_caller::<Environment>(accounts.eve);

            let mut contract = Polkapobal::new();
            let contract_address = contract.env().account_id();

            contract.register_member();

            let task1 = String::from("Task 1");
            let task2 = String::from("Task 2");
            contract.add_task(task1.clone());
            contract.add_task(task2.clone());

            assert_eq!(contract.active_tasks.len(), 2);
            assert_eq!(contract.task_info.get(&task1).unwrap(), (false, 0));
            assert_eq!(contract.task_info.get(&task2).unwrap(), (false, 0));

            set_balance(accounts.eve, 100);
            set_balance(contract_address, 0);

            ink::env::pay_with_call!(contract.fund_task(task1.clone()), 10);
            ink::env::pay_with_call!(contract.fund_task(task2.clone()), 20);

            contract.remove_task(task1.clone());

            assert_eq!(contract.active_tasks.len(), 1);
            assert_eq!(contract.task_info.get(&task1), None);
            assert_eq!(contract.unclaimed_funds, 10);
            assert_eq!(get_balance(contract_address), 30);

            contract.remove_task(task2.clone());

            assert_eq!(contract.active_tasks.len(), 0);
            assert_eq!(contract.task_info.get(&task2), None);
            assert_eq!(contract.unclaimed_funds, 30);
            assert_eq!(get_balance(contract_address), 30);

            assert_eq!(test::recorded_events().count(), 7);
        }

        #[ink::test]
        fn clear_tasks_works() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            ink::env::test::set_caller::<Environment>(accounts.eve);

            let mut contract = Polkapobal::new();
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

            assert_eq!(contract.active_tasks.len(), 3);

            contract.clear_tasks();

            assert_eq!(contract.active_tasks.len(), 0);

            assert!(!contract.task_info.contains(&task1));
            assert!(!contract.task_info.contains(&task2));
            assert!(!contract.task_info.contains(&task3));
            assert_eq!(contract.unclaimed_funds, 30);
            assert_eq!(get_balance(contract_address), 30);

            assert_eq!(test::recorded_events().count(), 7);
        }

        #[ink::test]
        fn fund_task_works() {
            let mut contract = Polkapobal::new();

            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

            let contract_address = contract.env().account_id();

            ink::env::test::set_caller::<Environment>(accounts.eve);

            contract.register_member();

            let task1 = String::from("Task 1");
            contract.add_task(task1.clone());

            assert_eq!(contract.active_tasks.len(), 1);
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
        #[should_panic(expected = "Member already exists")]
        fn register_member_panics() {
            let mut contract = Polkapobal::new();

            contract.register_member();
            // Should panic here
            contract.register_member();
        }

        #[ink::test]
        #[should_panic(expected = "Must be a member to call")]
        fn deregister_member_panics() {
            let mut contract = Polkapobal::new();

            contract.deregister_member();
        }

        #[ink::test]
        #[should_panic(expected = "Only owner can call")]
        fn clear_members_panics() {
            let mut contract = Polkapobal::new();

            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

            ink::env::test::set_caller::<Environment>(accounts.bob);
            contract.clear_members();
        }

        #[ink::test]
        #[should_panic(expected = "Task already exists")]
        fn add_task_twice_panics() {
            let mut contract = Polkapobal::new();

            contract.register_member();

            let task = String::from("Task");
            contract.add_task(task.clone());
            contract.add_task(task);
        }

        #[ink::test]
        #[should_panic(expected = "Must be a member to call")]
        fn add_task_when_not_member_panics() {
            let mut contract = Polkapobal::new();

            let task = String::from("Task");
            contract.add_task(task.clone());
        }

        #[ink::test]
        #[should_panic(expected = "Task does not exist")]
        fn remove_nonexistent_task_panics() {
            let mut contract = Polkapobal::new();

            contract.register_member();

            let task = String::from("Task");
            // task does not exist
            contract.remove_task(task);
        }

        #[ink::test]
        #[should_panic(expected = "Only owner can call")]
        fn remove_task_when_not_member_panics() {
            let mut contract = Polkapobal::new();

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
            let mut contract = Polkapobal::new();

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
            let mut contract = Polkapobal::new();

            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

            ink::env::test::set_caller::<Environment>(accounts.eve);

            contract.register_member();

            set_balance(accounts.eve, 100);

            let task = String::from("Task");
            // task does not exist
            ink::env::pay_with_call!(contract.fund_task(task), 10);
        }
    }
}
