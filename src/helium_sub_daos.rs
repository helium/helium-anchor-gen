use crate::voter_stake_registry::{LockupTrait, LockupKind as VsrLockUpKind, PositionV0, VotingMintConfig, VotingMintConfigV0};
pub use ::helium_sub_daos::*;
use anchor_lang::prelude::*;
use std::cmp::min;

pub type Result<T> = std::result::Result<T, Error>;

pub const EPOCH_LENGTH: i64 = 24 * 60 * 60;

pub fn current_epoch(unix_timestamp: i64) -> u64 {
    (unix_timestamp / (EPOCH_LENGTH)).try_into().unwrap()
}

pub fn next_epoch_ts(unix_timestamp: i64) -> u64 {
    (current_epoch(unix_timestamp) + 1) * u64::try_from(EPOCH_LENGTH).unwrap()
}

pub const FALL_RATE_FACTOR: u128 = 1_000_000_000_000;

pub trait PrecisePosition {
    fn voting_power_precise(
        &self,
        voting_mint_config: &VotingMintConfigV0,
        curr_ts: i64,
    ) -> Result<u128>;
    fn voting_power_precise_locked_precise(
        &self,
        curr_ts: i64,
        max_locked_vote_weight: u128,
        lockup_saturation_secs: u64,
    ) -> Result<u128>;

    fn voting_power_precise_cliff_precise(
        &self,
        curr_ts: i64,
        max_locked_vote_weight: u128,
        lockup_saturation_secs: u64,
    ) -> Result<u128>;
}

impl PrecisePosition for PositionV0 {
    fn voting_power_precise(
        &self,
        voting_mint_config: &VotingMintConfigV0,
        curr_ts: i64,
    ) -> Result<u128> {
        let baseline_vote_weight = (voting_mint_config
            .baseline_vote_weight(self.amount_deposited_native)?)
            .checked_mul(FALL_RATE_FACTOR)
            .unwrap();
        let max_locked_vote_weight =
            voting_mint_config.max_extra_lockup_vote_weight(self.amount_deposited_native)?;
        let genesis_multiplier =
            if curr_ts < self.genesis_end && voting_mint_config.genesis_vote_power_multiplier > 0 {
                voting_mint_config.genesis_vote_power_multiplier
            } else {
                1
            };

        let locked_vote_weight = self.voting_power_precise_locked_precise(
            curr_ts,
            max_locked_vote_weight,
            voting_mint_config.lockup_saturation_secs,
        )?;

        locked_vote_weight
            .checked_add(baseline_vote_weight)
            .unwrap()
            .checked_mul(genesis_multiplier as u128)
            .ok_or(error!(ErrorCode::ArithmeticError))
    }

    /// Vote power contribution from locked funds only.
    fn voting_power_precise_locked_precise(
        &self,
        curr_ts: i64,
        max_locked_vote_weight: u128,
        lockup_saturation_secs: u64,
    ) -> Result<u128> {
        if self.lockup.expired(curr_ts) || (max_locked_vote_weight == 0) {
            return Ok(0);
        }

        match self.lockup.kind {
            VsrLockUpKind::None => Ok(0),
            VsrLockUpKind::Cliff => self.voting_power_precise_cliff_precise(
                curr_ts,
                max_locked_vote_weight,
                lockup_saturation_secs,
            ),
            VsrLockUpKind::Constant => self.voting_power_precise_cliff_precise(
                curr_ts,
                max_locked_vote_weight,
                lockup_saturation_secs,
            ),
        }
    }

    fn voting_power_precise_cliff_precise(
        &self,
        curr_ts: i64,
        max_locked_vote_weight: u128,
        lockup_saturation_secs: u64,
    ) -> Result<u128> {
        let remaining = min(self.lockup.seconds_left(curr_ts), lockup_saturation_secs);
        Ok(
            (max_locked_vote_weight)
                .checked_mul(remaining as u128)
                .unwrap()
                .checked_mul(FALL_RATE_FACTOR)
                .unwrap()
                .checked_div(lockup_saturation_secs as u128)
                .unwrap(),
        )
    }
}

pub fn calculate_fall_rate(curr_vp: u128, future_vp: u128, num_seconds: u64) -> Option<u128> {
    if num_seconds == 0 {
        return Some(0);
    }

    let diff: u128 = curr_vp.checked_sub(future_vp).unwrap();

    diff.checked_div(num_seconds.into())
}

#[derive(Debug)]
pub struct VehntInfo {
    pub has_genesis: bool,
    pub vehnt_at_curr_ts: u128,
    pub pre_genesis_end_fall_rate: u128,
    pub post_genesis_end_fall_rate: u128,
    pub genesis_end_vehnt_correction: u128,
    pub genesis_end_fall_rate_correction: u128,
    pub end_vehnt_correction: u128,
    pub end_fall_rate_correction: u128,
}

pub fn caclulate_vhnt_info(
    curr_ts: i64,
    position: &PositionV0,
    voting_mint_config: &VotingMintConfigV0,
) -> Result<VehntInfo> {
    let vehnt_at_curr_ts = position.voting_power_precise(voting_mint_config, curr_ts)?;

    let has_genesis = position.genesis_end > curr_ts;
    let seconds_to_genesis = if has_genesis {
        u64::try_from(
            position
                .genesis_end
                .checked_sub(curr_ts)
                .unwrap()
                // Genesis end is inclusive (the genesis will go away at exactly genesis end), so subtract 1 second
                // We want to calculate the fall rates before genesis ends
                .checked_sub(1)
                .unwrap(),
        )
            .unwrap()
    } else {
        0
    };
    let seconds_from_genesis_to_end = if has_genesis {
        u64::try_from(
            position
                .lockup
                .end_ts
                .checked_sub(position.genesis_end)
                .unwrap(),
        )
            .unwrap()
    } else {
        position.lockup.seconds_left(curr_ts)
    };
    // One second before genesis end, the last moment we have the multiplier
    let vehnt_at_genesis_end = position.voting_power_precise(
        voting_mint_config,
        curr_ts
            .checked_add(i64::try_from(seconds_to_genesis).unwrap())
            .unwrap(),
    )?;
    let vehnt_at_genesis_end_exact = if has_genesis {
        position.voting_power_precise(voting_mint_config, position.genesis_end)?
    } else {
        position.voting_power_precise(voting_mint_config, curr_ts)?
    };
    let vehnt_at_position_end =
        position.voting_power_precise(voting_mint_config, position.lockup.end_ts)?;

    let pre_genesis_end_fall_rate =
        calculate_fall_rate(vehnt_at_curr_ts, vehnt_at_genesis_end, seconds_to_genesis).unwrap();
    let post_genesis_end_fall_rate = calculate_fall_rate(
        vehnt_at_genesis_end_exact,
        vehnt_at_position_end,
        seconds_from_genesis_to_end,
    )
        .unwrap();

    let mut genesis_end_vehnt_correction = 0;
    let mut genesis_end_fall_rate_correction = 0;
    if has_genesis {
        let genesis_end_epoch_start_ts =
            i64::try_from(current_epoch(position.genesis_end)).unwrap() * EPOCH_LENGTH;

        if let VsrLockUpKind::Cliff = position.lockup.kind {
            genesis_end_fall_rate_correction = pre_genesis_end_fall_rate
                .checked_sub(post_genesis_end_fall_rate)
                .unwrap();
        }

        // Subtract the genesis bonus from the vehnt.
        // When we do this, we're overcorrecting because the fall rate (corrected to post-genesis)
        // is also taking off vehnt for the time period between closing info start and genesis end.
        // So add that fall rate back in.
        // Only do this if the genesis end epoch isn't the same as the position end epoch.
        // If these are the same, then the full vehnt at epoch start is already being taken off.
        let constant = matches!(position.lockup.kind, VsrLockUpKind::Constant);

        if constant || current_epoch(position.genesis_end) != current_epoch(position.lockup.end_ts)
        {
            // edge case, if the genesis end is _exactly_ the start of the epoch, getting the voting power at the epoch start
            // will not include the genesis. When this happens, we'll miss a vehnt correction
            if genesis_end_epoch_start_ts == position.genesis_end {
                genesis_end_vehnt_correction = position
                    .voting_power_precise(voting_mint_config, genesis_end_epoch_start_ts - 1)?
                    .checked_sub(vehnt_at_genesis_end_exact)
                    .unwrap();
            } else {
                genesis_end_vehnt_correction = position
                    .voting_power_precise(voting_mint_config, genesis_end_epoch_start_ts)?
                    .checked_sub(vehnt_at_genesis_end_exact)
                    .unwrap()
                    // Correction factor
                    .checked_sub(
                        post_genesis_end_fall_rate
                            .checked_mul(
                                u128::try_from(
                                    position
                                        .genesis_end
                                        .checked_sub(genesis_end_epoch_start_ts)
                                        .unwrap(),
                                )
                                    .unwrap(),
                            )
                            .unwrap(),
                    )
                    .unwrap();
            }
        }
    }

    let mut end_fall_rate_correction = 0;
    let mut end_vehnt_correction = 0;
    if let VsrLockUpKind::Cliff = position.lockup.kind {
        let end_epoch_start_ts =
            i64::try_from(current_epoch(position.lockup.end_ts)).unwrap() * EPOCH_LENGTH;
        let vehnt_at_closing_epoch_start =
            position.voting_power_precise(voting_mint_config, end_epoch_start_ts)?;

        end_vehnt_correction = vehnt_at_closing_epoch_start;
        end_fall_rate_correction = post_genesis_end_fall_rate;
    }

    Ok(VehntInfo {
        has_genesis,
        pre_genesis_end_fall_rate,
        post_genesis_end_fall_rate,
        vehnt_at_curr_ts,
        genesis_end_fall_rate_correction,
        genesis_end_vehnt_correction,
        end_fall_rate_correction,
        end_vehnt_correction,
    })
}

pub trait SubDaoEpochInfoV0Trait {
    fn start_ts(&self) -> i64;
    fn end_ts(&self) -> i64;
}

impl SubDaoEpochInfoV0Trait for SubDaoEpochInfoV0 {
    fn start_ts(&self) -> i64 {
        i64::try_from(self.epoch).unwrap() * EPOCH_LENGTH
    }

    fn end_ts(&self) -> i64 {
        i64::try_from(self.epoch + 1).unwrap() * EPOCH_LENGTH
    }
}

#[error_code]
pub enum ErrorCode {
    #[msg("The realloc increase was too large")]
    InvalidDataIncrease,

    #[msg("Error in arithmetic")]
    ArithmeticError,

    #[msg("Utility score was already calculated for this sub dao")]
    UtilityScoreAlreadyCalculated,

    #[msg("Cannot calculate rewards until the epoch is over")]
    EpochNotOver,

    #[msg("All utility scores must be calculated before rewards can be issued")]
    MissingUtilityScores,

    #[msg("The subdao does not have a utility score")]
    NoUtilityScore,

    #[msg("Not enough veHNT")]
    NotEnoughVeHnt,

    #[msg("Lockup hasn't expired yet")]
    LockupNotExpired,

    #[msg("This staking position has already been purged")]
    PositionAlreadyPurged,

    #[msg("This position is healthy, refresh not needed")]
    RefreshNotNeeded,

    #[msg("Failed to calculate the voting power")]
    FailedVotingPowerCalculation,

    #[msg("Rewards need to be claimed in the correct epoch order")]
    InvalidClaimEpoch,

    #[msg("Epochs start after the earliest emission schedule")]
    EpochTooEarly,

    #[msg("Must calculate vehnt linearly. Please ensure the previous epoch has been completed")]
    MustCalculateVehntLinearly,

    #[msg("Cannot change a position while it is delegated")]
    PositionChangeWhileDelegated,

    #[msg("This epoch was not closed, cannot claim rewards.")]
    EpochNotClosed,

    #[msg("Cannot delegate on a position ending this epoch")]
    NoDelegateEndingPosition,
}
