# 0003 Per-member share cap is relative to `max_units`

**Context.** The brief forbids any member holding more than 50% of a pool unless they are the only member. Checking the live share on every commit is O(members) or needs stale tracking, and makes early small joiners impossible once a whale commits.

**Decision.** A member (other than the sole member) may hold at most `max_member_share_bp` of the pool's `max_units`. O(1), deterministic.

This cap applies equally to all participants, including the pool organizer. While a participant (including the organizer) is the sole member (`member_count == 0` or sole existing member modifying their commitment), the share cap is exempted to allow initial bootstrapping. Once any other member exists in the pool, the cap is strictly enforced: any commitment exceeding `max_member_share_bp` of `max_units` is rejected with `Error::OverMemberShare`.

**Consequences.** A single large member plus one tiny sybil can still reach a tier; trader tier caps and KYC bound that.
