typedef bool check_t;

int main() {
  for (char i = 0; i != 3; i++) {
    check_t check = i == 2;
    if (check) {
      putchar('a');
    }

    putchar('h' + i);
  }
}
