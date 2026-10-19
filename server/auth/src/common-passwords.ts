// A short list, not a breach corpus. It targets what survives a 10-character minimum:
// keyboard walks, digit runs, and a common word padded with digits or symbols. For breach
// checking, plug a lookup into `passwordCheck`.

export const COMMON_PASSWORDS: ReadonlySet<string> = new Set([
  "1234567890", "0123456789", "12345678910", "1234567890a", "123456789a", "123456789q",
  "a123456789", "q123456789", "0987654321", "9876543210", "1472583690", "1122334455",
  "1234512345", "1234554321", "1212121212", "1231231234", "1q2w3e4r5t", "1q2w3e4r5t6y",
  "q1w2e3r4t5", "q1w2e3r4t5y6", "1qaz2wsx3edc", "1qazxsw23edc", "zaq12wsxcde3", "zaq1zaq1zaq1",
  "qazwsxedc123", "qazwsxedcrfv", "123qweasdzxc", "qweasdzxc123", "qwertyuiop", "qwertyuiop1",
  "qwertyuiop123", "qwerty1234", "qwerty12345", "qwerty123456", "qwertyuiop[]", "asdfghjkl1",
  "asdfghjkl;", "asdfghjkl123", "asdfghjkl;'", "zxcvbnm123", "zxcvbnm1234", "zxcvbnm,./",
  "1qaz@wsx3edc", "!qaz2wsx", "abcdefghij", "abcdefghijk", "abc1234567", "abcd123456",
  "abcde12345", "abcdef1234", "a1b2c3d4e5", "a1b2c3d4e5f6", "aaaaaaaaaa", "1111111111",
  "0000000000", "2222222222", "5555555555", "7777777777", "8888888888", "9999999999",
  "password12", "password123", "password1234", "password12345", "password123!", "password!!",
  "passw0rd123", "p@ssw0rd123", "p@ssword123", "mypassword", "mypassword1", "mypassword123",
  "iloveyou12", "iloveyou123", "iloveyou1234", "loveyou123", "letmein123", "letmein1234",
  "welcome123", "welcome1234", "welcome2024", "welcome2025", "welcome2026", "changeme123",
  "administrator", "admin12345", "admin123456", "computer123", "internet123", "starwars123",
  "pokemon123", "sunshine123", "princess123", "football123", "baseball123", "basketball",
  "basketball1", "superman123", "batman1234", "monkey12345", "dragon1234", "shadow1234",
  "michael123", "jennifer12", "trustno1234", "master1234", "liverpool1", "chelsea123",
  "manchester", "manchesterunited", "teacher123", "teacher1234", "student123", "student1234",
  "school1234", "classroom1", "education1", "summer2024", "summer2025", "summer2026",
  "winter2024", "winter2025", "winter2026", "spring2025", "spring2026", "autumn2025",
  "password2024", "password2025", "password2026", "correcthorsebatterystaple",
]);

/** Rejected when they are the whole password apart from leading/trailing digits and symbols. */
export const COMMON_BASE_WORDS: ReadonlySet<string> = new Set([
  "password", "passw", "passwd", "passwrd", "passwor", "pass", "qwerty", "qwertyuiop",
  "asdfgh", "asdfghjkl", "zxcvbnm", "iloveyou", "letmein", "welcome", "admin",
  "administrator", "changeme", "monkey", "dragon", "sunshine", "princess", "football",
  "baseball", "basketball", "soccer", "master", "shadow", "superman", "batman", "secret",
  "abc", "abcdef", "abcdefg", "qazwsx", "teacher", "student", "school", "summer", "winter",
  "spring", "autumn", "login", "user", "test", "default", "hello", "freedom", "whatever",
]);
