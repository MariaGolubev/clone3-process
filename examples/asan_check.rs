fn main() {
    let mut data = vec![0u8; 4];

    unsafe {
        let ptr = data.as_mut_ptr();
        *ptr.add(4) = 1; // Намеренный выход за границы массива
    }
}
