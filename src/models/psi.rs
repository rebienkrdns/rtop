#[derive(Debug, Clone, PartialEq, Default)]
pub struct PsiValues {
    pub avg10: f64,
    pub avg60: f64,
    pub avg300: f64,
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PsiData {
    pub cpu_some: PsiValues,
    pub memory_some: PsiValues,
    pub memory_full: PsiValues,
    pub io_some: PsiValues,
    pub io_full: PsiValues,
}
