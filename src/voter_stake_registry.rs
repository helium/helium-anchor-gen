pub use ::voter_stake_registry::*;
use std::cmp::min;

pub const PRECISION_FACTOR: u128 = 1_000_000_000_000;

const SCALED_FACTOR_BASE: u64 = 1_000_000_000;

use anchor_lang::prelude::*;
pub type Result<T> = std::result::Result<T, Error>;

pub trait PositionV0Trait {
    fn voting_power(&self, voting_mint_config: &VotingMintConfigV0, curr_ts: i64) -> Result<u128>;

    fn voting_power_locked(
        &self,
        curr_ts: i64,
        max_locked_vote_weight: u128,
        lockup_saturation_secs: u64,
    ) -> Result<u128>;

    fn voting_power_cliff(
        &self,
        curr_ts: i64,
        max_locked_vote_weight: u128,
        lockup_saturation_secs: u64,
    ) -> Result<u128>;

    fn amount_unlocked(&self, curr_ts: i64) -> u64;

    fn amount_locked(&self, curr_ts: i64) -> u64;
}

impl PositionV0Trait for PositionV0 {
    // # Voting Power Calculation
    //
    // Returns the voting power for the position, giving locked tokens boosted
    // voting power that scales linearly with the lockup time.
    //
    // For each cliff-locked token, the vote weight is:
    //
    // ```
    //    voting_power = baseline_vote_weight
    //                   + lockup_duration_factor * max_extra_lockup_vote_weight
    // ```
    //
    // with
    //   - lockup_duration_factor = min(lockup_time_remaining / lockup_saturation_secs, 1)
    //   - the VotingMintConfig providing the values for
    //     baseline_vote_weight, max_extra_lockup_vote_weight, lockup_saturation_secs
    //
    // ## Cliff Lockup
    //
    // The cliff lockup allows one to lockup their tokens for a set period
    // of time, unlocking all at once on a given date.
    //
    // The calculation for this is straightforward and is detailed above.
    //
    // ### Decay
    //
    // As time passes, the voting power decays until it's back to just
    // fixed_factor when the cliff has passed. This is important because at
    // each point in time the lockup should be equivalent to a new lockup
    // made for the remaining time period.
    //
    fn voting_power(&self, voting_mint_config: &VotingMintConfigV0, curr_ts: i64) -> Result<u128> {
        let baseline_vote_weight =
            voting_mint_config.baseline_vote_weight(self.amount_deposited_native)?;
        let max_locked_vote_weight =
            voting_mint_config.max_extra_lockup_vote_weight(self.amount_deposited_native)?;
        let genesis_multiplier =
            if curr_ts < self.genesis_end && voting_mint_config.genesis_vote_power_multiplier > 0 {
                voting_mint_config.genesis_vote_power_multiplier
            } else {
                1
            };

        let locked_vote_weight = self.voting_power_locked(
            curr_ts,
            max_locked_vote_weight,
            voting_mint_config.lockup_saturation_secs,
        )?;

        require_gte!(
            max_locked_vote_weight,
            locked_vote_weight,
            VsrError::InternalErrorBadLockupVoteWeight
        );

        baseline_vote_weight
            .checked_add(locked_vote_weight)
            .unwrap()
            .checked_mul(genesis_multiplier as u128)
            .ok_or_else(|| error!(VsrError::VoterWeightOverflow))
    }

    // Vote power contribution from locked funds only.
    fn voting_power_locked(
        &self,
        curr_ts: i64,
        max_locked_vote_weight: u128,
        lockup_saturation_secs: u64,
    ) -> Result<u128> {
        if self.lockup.expired(curr_ts) || (max_locked_vote_weight == 0) {
            return Ok(0);
        }

        match self.lockup.kind {
            LockupKind::None => Ok(0),
            LockupKind::Cliff => {
                self.voting_power_cliff(curr_ts, max_locked_vote_weight, lockup_saturation_secs)
            }
            LockupKind::Constant => {
                self.voting_power_cliff(curr_ts, max_locked_vote_weight, lockup_saturation_secs)
            }
        }
    }

    fn voting_power_cliff(
        &self,
        curr_ts: i64,
        max_locked_vote_weight: u128,
        lockup_saturation_secs: u64,
    ) -> Result<u128> {
        let remaining = min(self.lockup.seconds_left(curr_ts), lockup_saturation_secs);
        Ok(max_locked_vote_weight
            .checked_mul(remaining as u128)
            .unwrap()
            .checked_div(lockup_saturation_secs as u128)
            .unwrap())
    }

    fn amount_unlocked(&self, curr_ts: i64) -> u64 {
        if self.lockup.end_ts <= curr_ts {
            self.amount_deposited_native
        } else {
            0
        }
    }

    fn amount_locked(&self, curr_ts: i64) -> u64 {
        self.amount_deposited_native
            .checked_sub(self.amount_unlocked(curr_ts))
            .unwrap()
    }
}

pub trait VotingMintConfig {
    fn apply_factor(base: u64, factor: u64) -> Result<u128>;

    fn baseline_vote_weight(&self, amount_native: u64) -> Result<u128>;

    fn max_extra_lockup_vote_weight(&self, amount_native: u64) -> Result<u128>;
    fn in_use(&self) -> bool;
}

impl VotingMintConfig for VotingMintConfigV0 {
    /// Apply a factor in SCALED_FACTOR_BASE units.
    fn apply_factor(base: u64, factor: u64) -> Result<u128> {
        let compute = || -> Option<u128> {
            (base as u128)
                .checked_mul(factor as u128)?
                .checked_div(SCALED_FACTOR_BASE as u128)
        };
        compute().ok_or_else(|| error!(VsrError::VoterWeightOverflow))
    }

    /// The vote weight a deposit of a number of native tokens should have.
    ///
    /// This vote_weight is a component for all funds in a voter account, no
    /// matter if locked up or not.//
    fn baseline_vote_weight(&self, amount_native: u64) -> Result<u128> {
        Self::apply_factor(amount_native, self.baseline_vote_weight_scaled_factor)
    }

    /// The maximum extra vote weight a number of locked up native tokens can have.
    /// Will be multiplied with a factor between 0 and 1 for the lockup duration.
    fn max_extra_lockup_vote_weight(&self, amount_native: u64) -> Result<u128> {
        Self::apply_factor(
            amount_native,
            self.max_extra_lockup_vote_weight_scaled_factor,
        )
    }

    /// Whether this voting mint is configured.
    fn in_use(&self) -> bool {
        self.mint != Pubkey::default()
    }
}

pub const MAX_LOCKUP_PERIODS: u32 = 365 * 200;
pub const MAX_LOCKUP_IN_FUTURE_SECS: i64 = 100 * 365 * 24 * 60 * 60;
pub const SECS_PER_DAY: u64 = 86_400;

pub trait LockupTrait {
    fn new_from_periods(
        kind: LockupKind,
        curr_ts: i64,
        start_ts: i64,
        periods: u32,
    ) -> Result<Self>
    where
        Self: Sized;
    fn expired(&self, curr_ts: i64) -> bool;
    fn total_seconds(&self) -> u64;
    fn seconds_left(&self, curr_ts: i64) -> u64;
    fn seconds_since_expiry(&self, curr_ts: i64) -> u64;
}

impl LockupTrait for Lockup {
    /// Create lockup for a given period
    fn new_from_periods(
        kind: LockupKind,
        curr_ts: i64,
        start_ts: i64,
        periods: u32,
    ) -> Result<Self> {
        require_gt!(
            curr_ts + MAX_LOCKUP_IN_FUTURE_SECS,
            start_ts,
            VsrError::DepositStartTooFarInFuture
        );
        require_gte!(MAX_LOCKUP_PERIODS, periods, VsrError::InvalidLockupPeriod);
        let period_secs = if matches!(kind, LockupKind::None) {
            0
        } else {
            SECS_PER_DAY
        };

        Ok(Self {
            kind,
            start_ts,
            end_ts: start_ts
                .checked_add(
                    i64::try_from((periods as u64).checked_mul(period_secs).unwrap()).unwrap(),
                )
                .unwrap(),
        })
    }

    /// True when the lockup is finished.
    fn expired(&self, curr_ts: i64) -> bool {
        self.seconds_left(curr_ts) == 0
    }

    // Total seconds in lockup
    fn total_seconds(&self) -> u64 {
        (self.end_ts - self.start_ts) as u64
    }

    /// Number of seconds left in the lockup.
    /// May be more than end_ts-start_ts if curr_ts < start_ts.
    fn seconds_left(&self, mut curr_ts: i64) -> u64 {
        if matches!(self.kind, LockupKind::Constant) {
            curr_ts = self.start_ts;
        }
        if curr_ts >= self.end_ts {
            0
        } else {
            (self.end_ts - curr_ts) as u64
        }
    }

    /// Number of seconds since the lockup expired.
    /// Returns 0 if the lockup hasn't expired
    fn seconds_since_expiry(&self, curr_ts: i64) -> u64 {
        if !self.expired(curr_ts) {
            return 0;
        }
        (curr_ts - self.end_ts) as u64
    }
}

#[error_code]
pub enum VsrError {
    // 6000 / 0x1770
    #[msg("Exchange rate must be greater than zero")]
    InvalidRate,
    // 6001 / 0x1771
    #[msg("")]
    RatesFull,
    // 6002 / 0x1772
    #[msg("")]
    VotingMintNotFound,
    // 6003 / 0x1773
    #[msg("")]
    DepositEntryNotFound,
    // 6004 / 0x1774
    #[msg("")]
    DepositEntryFull,
    // 6005 / 0x1775
    #[msg("")]
    VotingTokenNonZero,
    // 6006 / 0x1776
    #[msg("")]
    OutOfBoundsDepositEntryIndex,
    // 6007 / 0x1777
    #[msg("")]
    UnusedDepositEntryIndex,
    // 6008 / 0x1778
    #[msg("")]
    InsufficientUnlockedTokens,
    // 6009 / 0x1779
    #[msg("")]
    UnableToConvert,
    // 6010 / 0x177a
    #[msg("")]
    InvalidLockupPeriod,
    // 6011 / 0x177b
    #[msg("")]
    InvalidEndTs,
    // 6012 / 0x177c
    #[msg("")]
    InvalidDays,
    // 6013 / 0x177d
    #[msg("")]
    VotingMintConfigIndexAlreadyInUse,
    // 6014 / 0x177e
    #[msg("")]
    OutOfBoundsVotingMintConfigIndex,
    // 6015 / 0x177f
    #[msg("Exchange rate decimals cannot be larger than registrar decimals")]
    InvalidDecimals,
    // 6016 / 0x1780
    #[msg("")]
    InvalidToDepositAndWithdrawInOneSlot,
    // 6017 / 0x1781
    #[msg("")]
    ShouldBeTheFirstIxInATx,
    // 6018 / 0x1782
    #[msg("")]
    ForbiddenCpi,
    // 6019 / 0x1783
    #[msg("")]
    InvalidMint,
    // 6020 / 0x1784
    #[msg("")]
    DebugInstruction,
    // 6021 / 0x1785
    #[msg("")]
    ClawbackNotAllowedOnDeposit,
    // 6022 / 0x1786
    #[msg("")]
    DepositStillLocked,
    // 6023 / 0x1787
    #[msg("")]
    InvalidAuthority,
    // 6024 / 0x1788
    #[msg("")]
    InvalidTokenOwnerRecord,
    // 6025 / 0x1789
    #[msg("")]
    InvalidRealmAuthority,
    // 6026 / 0x178a
    #[msg("")]
    VoterWeightOverflow,
    // 6027 / 0x178b
    #[msg("")]
    LockupSaturationMustBePositive,
    // 6028 / 0x178c
    #[msg("")]
    VotingMintConfiguredWithDifferentIndex,
    // 6029 / 0x178d
    #[msg("")]
    InternalProgramError,
    // 6030 / 0x178e
    #[msg("")]
    InsufficientLockedTokens,
    // 6031 / 0x178f
    #[msg("")]
    MustKeepTokensLocked,
    // 6032 / 0x1790
    #[msg("")]
    InvalidLockupKind,
    // 6033 / 0x1791
    #[msg("")]
    InvalidChangeToClawbackDepositEntry,
    // 6034 / 0x1792
    #[msg("")]
    InternalErrorBadLockupVoteWeight,
    // 6035 / 0x1793
    #[msg("")]
    DepositStartTooFarInFuture,
    // 6036 / 0x1794
    #[msg("")]
    VaultTokenNonZero,
    // 6037 / 0x1795
    #[msg("")]
    InvalidTimestampArguments,

    #[msg("Cast vote is not allowed on update_voter_weight_record_v0 endpoint")]
    CastVoteIsNotAllowed,

    #[msg("Program id was not what was expected")]
    InvalidProgramId,

    #[msg("")]
    InvalidMintOwner,
    #[msg("")]
    InvalidMintAmount,
    #[msg("")]
    DuplicatedNftDetected,
    #[msg("")]
    InvalidTokenOwnerForVoterWeightRecord,
    #[msg("")]
    NftAlreadyVoted,
    #[msg("")]
    InvalidProposalForNftVoteRecord,
    #[msg("")]
    InvalidTokenOwnerForNftVoteRecord,
    #[msg("")]
    UninitializedAccount,
    #[msg("")]
    PositionNotWritable,
    #[msg("")]
    InvalidVoteRecordForNftVoteRecord,
    #[msg("")]
    VoteRecordMustBeWithdrawn,
    #[msg("")]
    VoterWeightRecordMustBeExpired,
    #[msg("")]
    InvalidMintForPosition,
    #[msg("")]
    InvalidOwner,
    #[msg("You may not deposit additional tokens on a position created during the genesis period that still has the genesis multiplier")]
    NoDepositOnGenesisPositions,
    #[msg("Cannot change a position while active votes exist")]
    ActiveVotesExist,
    #[msg("Position update authority must sign off on this transaction")]
    UnauthorizedPositionUpdateAuthority,
    #[msg("Cannot transfer to the same position")]
    SamePosition,
}
