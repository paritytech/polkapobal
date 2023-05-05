#![cfg_attr(not(feature = "std"), no_std)]

#[ink::contract]
mod polkapobal {
    use ink::{
        prelude::vec::Vec,
        storage::Mapping,
    };

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

    #[ink(storage)]
    pub struct Polkapobal {
        owner: AccountId,
        members: Vec<AccountId>,
        is_member: Mapping<AccountId, ()>,
    }

    impl Polkapobal {
        #[ink(constructor)]
        pub fn new() -> Self {
            Polkapobal {
                owner: Self::env().caller(),
                members: Vec::new(),
                is_member: Mapping::default(),
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
            assert!(self.is_member.contains(&caller), "Member does not exist");
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
            let caller = self.env().caller();

            // Ensure that the caller is the owner
            assert_eq!(caller, self.owner, "Only owner can clear members");

            // Iterate over `members` vec and remove each member.
            // Done in reverse so the member indices' do not change.
            for (i, member) in self.members.clone().iter().enumerate().rev() {
                self.is_member.remove(member);
                self.members.swap_remove(i);
            }

            self.env().emit_event(MembersCleared {});
        }
    }

    #[cfg(test)]
    mod tests {
        /// Imports all the definitions from the outer scope so we can use them here.
        use super::*;
        use ink::env::test;

        #[ink::test]
        fn construction_works() {
            let expected = Polkapobal {
                owner: AccountId::from([0x01; 32]),
                members: Vec::new(),
                is_member: Mapping::default(),
            };

            let contract = Polkapobal::new();
            assert_eq!(contract.owner, expected.owner);
            assert_eq!(contract.members.len(), 0);
        }

        #[ink::test]
        fn register_member_works() {
            let mut contract = Polkapobal::new();

            let accounts =
                ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

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

            let accounts =
                ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

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

            let accounts =
                ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

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
        #[should_panic(expected = "Member already exists")]
        fn register_member_panics() {
            let mut contract = Polkapobal::new();

            contract.register_member();
            // Should panic here
            contract.register_member();
        }

        #[ink::test]
        #[should_panic(expected = "Member does not exist")]
        fn deregister_member_panics() {
            let mut contract = Polkapobal::new();

            contract.deregister_member();
        }

        #[ink::test]
        #[should_panic(expected = "Only owner can clear members")]
        fn clear_members_panics() {
            let mut contract = Polkapobal::new();

            let accounts =
                ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

            ink::env::test::set_caller::<Environment>(accounts.bob);
            contract.clear_members();
        }
    }
}
