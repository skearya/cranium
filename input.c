int main()
{
    char i = 3;

    while (i)
    {
        putc('a' + i);
        i = i - 1;
    }
    
    putc('\n');
}
