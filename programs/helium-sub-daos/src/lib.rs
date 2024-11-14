anchor_gen::generate_cpi_crate!("../../idl/helium_sub_daos.json");
declare_id!("hdaoVTCqhfHHo75XdAMxBKdUqvq1i5bF23sisBqVgGR");

impl Default for Curve {
    fn default() -> Self {
        Curve::ExponentialCurveV0 { k: 1 }
    }
}
